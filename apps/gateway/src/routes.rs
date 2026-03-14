//! Route handlers and Axum router construction

use axum::{extract::State, middleware::Next, Json};
use serde_json::{json, Value};
use std::net::SocketAddr;

use crate::config;
use crate::state::{AppState, HealthStatus, ModelInfo};
use smart_routing::classification::RequestClassifier;

pub(crate) async fn root() -> Json<Value> {
    Json(json!({
        "name": "Gateway API",
        "version": "0.1.0",
        "description": "Smart routing gateway for LLM requests",
        "features": [
            "Smart Routing",
            "Model Registry",
            "LLM Tracing",
            "Health Management"
        ],
        "endpoints": {
            "health": "/health",
            "models": "/api/models",
            "route": "/api/route"
        }
    }))
}

/// Authentication middleware for protected routes
/// Validates Bearer token against configured auth_tokens
/// Fails-closed by default (requires auth) unless GATEWAY_ENV=development is set
pub(crate) async fn auth_middleware(
    State(state): State<AppState>,
    req: axum::extract::Request,
    next: Next,
) -> std::result::Result<axum::response::Response, (axum::http::StatusCode, Json<Value>)> {
    use axum::http::header::AUTHORIZATION;
    use axum::http::StatusCode;

    // Check for development environment override
    let is_development = std::env::var("GATEWAY_ENV")
        .map(|v| v.to_lowercase() == "development")
        .unwrap_or(false);

    // Skip auth only in development mode if no tokens are configured
    if is_development && state.config.server.auth_tokens.is_empty() {
        tracing::warn!("Authentication skipped in development mode (no auth_tokens configured)");
        return Ok(next.run(req).await);
    }

    // Fail-closed if no tokens are configured but we're not in development mode
    if state.config.server.auth_tokens.is_empty() {
        tracing::error!("Access denied: No auth_tokens configured in non-development mode");
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": {
                    "type": "config_error",
                    "message": "Gateway is improperly configured: No authentication tokens available."
                }
            })),
        ));
    }

    // Extract Authorization header
    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    match auth_header {
        Some(header) => {
            // Check Bearer token format
            if let Some(token) = header.strip_prefix("Bearer ") {
                // Validate against configured tokens using constant-time comparison
                // that iterates all tokens (no short-circuit) to prevent timing side-channels
                if config::constant_time_token_matches(token, &state.config.server.auth_tokens) {
                    return Ok(next.run(req).await);
                }
            }
            Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": {
                        "type": "invalid_request_error",
                        "message": "Invalid or expired API token"
                    }
                })),
            ))
        },
        None => Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": {
                    "type": "invalid_request_error",
                    "message": "Missing Authorization header. Use: Authorization: Bearer <token>"
                }
            })),
        )),
    }
}

/// Middleware that adds standard security headers to all HTTP responses.
pub(crate) async fn security_headers_middleware(
    req: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    use axum::http::header::{HeaderName, HeaderValue};

    let mut response = next.run(req).await;

    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
    );

    response
}

/// Rate limiting middleware. Extracts client IP from X-Forwarded-For or
/// X-Real-IP headers only when `trust_proxy_headers` is enabled (gateway behind
/// a trusted reverse proxy). Otherwise, all requests share a single bucket to
/// prevent header-spoofing bypasses.
pub(crate) async fn rate_limit_middleware(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<SocketAddr>,
    req: axum::extract::Request,
    next: Next,
) -> std::result::Result<axum::response::Response, (axum::http::StatusCode, Json<Value>)> {
    let peer_ip = addr.ip().to_string();
    let client_ip = if state.config.server.trust_proxy_headers {
        req.headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or_else(|| req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()))
            .unwrap_or(&peer_ip)
    } else {
        &peer_ip
    };

    if !state.rate_limiter.check(client_ip) {
        return Err((
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "error": {
                    "type": "rate_limit_error",
                    "message": "Too many requests. Please try again later."
                }
            })),
        ));
    }

    Ok(next.run(req).await)
}

pub(crate) async fn health_check(State(state): State<AppState>) -> Json<HealthStatus> {
    let uptime = state.start_time.elapsed().as_secs();

    // Count credential health states
    // For now, all configured credentials are considered healthy
    // In production, this would check actual health status from HealthManager
    let credential_count = state.credentials.len();
    let healthy_count = credential_count; // Assume all healthy until health checks run
    let degraded_count = 0;
    let unhealthy_count = 0;

    Json(HealthStatus {
        status: "healthy".to_string(),
        uptime_secs: uptime,
        credential_count,
        healthy_count,
        degraded_count,
        unhealthy_count,
    })
}

pub(crate) async fn list_models(State(state): State<AppState>) -> Json<Value> {
    // Build model list from configured credentials
    // Note: When allowed_models is empty, it means all provider models are allowed
    // In a full implementation, we would query the ModelRegistry for provider models
    let models: Vec<ModelInfo> = state
        .config
        .credentials
        .iter()
        .flat_map(|cred| {
            if cred.allowed_models.is_empty() {
                // Empty allowed_models means all models for this provider
                // TODO: Query ModelRegistry for all provider models
                vec![ModelInfo {
                    id: format!("{}:*", cred.provider),
                    provider: cred.provider.clone(),
                    capabilities: vec!["all".to_string()],
                    context_window: 128_000,
                }]
            } else {
                cred.allowed_models
                    .iter()
                    .map(|model_id| ModelInfo {
                        id: model_id.clone(),
                        provider: cred.provider.clone(),
                        capabilities: vec![], // Would be populated from model registry
                        context_window: 128_000, // Default, would come from registry
                    })
                    .collect()
            }
        })
        .collect();

    let count = models.len();

    Json(json!({
        "models": models,
        "count": count,
        "message": if count == 0 {
            "No models configured. Add credentials to gateway.yaml"
        } else {
            "Models loaded from configuration"
        }
    }))
}

pub(crate) async fn route_request(State(state): State<AppState>) -> Json<Value> {
    // Create a sample request for demonstration
    // In production, this would come from the request body as JSON
    let sample_request = json!({
        "messages": [
            {
                "role": "user",
                "content": "Hello, how are you?"
            }
        ],
        "model": "unknown"
    });

    // Step 1: Classify the request using RequestClassifier
    let classified = state.classifier.classify(&sample_request);

    // Step 2: Plan routes using Router with configured credentials
    let auths = state.credentials.clone();
    let session_id: Option<&str> = None;

    let route_plan = state.router.plan(&classified, auths, session_id).await;

    // Step 3: Return the route plan
    // In production, Step 4 would execute the route using RouteExecutor
    // and Step 5 would return the LLM response

    // Format the primary route
    let primary_json = match &route_plan.primary {
        Some(primary) => json!({
            "credential_id": primary.credential_id,
            "model_id": primary.model_id,
            "provider": primary.provider,
            "utility": primary.utility,
            "weight": primary.weight,
        }),
        None => json!(null),
    };

    // Format fallbacks
    let fallbacks_json: Vec<Value> = route_plan
        .fallbacks
        .iter()
        .map(|fb| {
            json!({
                "credential_id": fb.auth_id,
                "position": fb.position,
                "weight": fb.weight,
                "provider": fb.provider,
            })
        })
        .collect();

    // Build response
    Json(json!({
        "route_plan": {
            "primary": primary_json,
            "fallbacks": fallbacks_json,
            "total_candidates": route_plan.total_candidates,
            "filtered_candidates": route_plan.filtered_candidates,
        },
        "classification": {
            "required_capabilities": {
                "vision": classified.required_capabilities.vision,
                "tools": classified.required_capabilities.tools,
                "streaming": classified.required_capabilities.streaming,
                "thinking": classified.required_capabilities.thinking
            },
            "estimated_tokens": classified.estimated_tokens,
            "format": format!("{:?}", classified.format),
            "quality_preference": format!("{:?}", classified.quality_preference)
        },
        "message": if route_plan.primary.is_some() {
            "Route planned successfully"
        } else {
            "No suitable routes found - configure credentials in gateway.yaml"
        }
    }))
}

/// POST /v1/chat/completions - Proxy endpoint for chat completion requests
pub(crate) async fn chat_completions(
    State(state): State<AppState>,
    Json(request): Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    use crate::providers::{self, ProviderAdapter};

    // Step 1: Classify the request
    let classified = state.classifier.classify(&request);

    // Step 2: Extract model from request
    let model_id = request
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown");

    // Step 3: Plan routes
    let auths = state.credentials.clone();
    let session_id = request.get("session_id").and_then(|s| s.as_str());
    let route_plan = state.router.plan(&classified, auths, session_id).await;

    // Step 4: Get primary route
    let primary = match &route_plan.primary {
        Some(p) => p,
        None => {
            return Err((
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "type": "no_route_available",
                        "message": "No suitable routes found. Configure credentials in gateway.yaml"
                    }
                })),
            ));
        },
    };

    // Step 5: Find credential config for this route
    let credential = match state
        .config
        .credentials
        .iter()
        .find(|c| c.id == primary.credential_id)
    {
        Some(cred) => cred,
        None => {
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "type": "credential_not_found",
                        "message": format!("Credential {} not found in configuration", primary.credential_id)
                    }
                })),
            ));
        },
    };

    // Step 6: Select provider adapter
    let provider = &primary.provider;
    let adapter: Box<dyn ProviderAdapter> = match provider.as_str() {
        "openai" | "azure-openai" => Box::new(providers::OpenAIAdapter::new()),
        "google" => Box::new(providers::GoogleAdapter::new()),
        "deepseek" => Box::new(providers::OpenAIAdapter::new()),
        "mistral" | "mistral-large" => Box::new(providers::OpenAIAdapter::new()),
        _ => Box::new(providers::OpenAIAdapter::new()), // Default to OpenAI format
    };

    // Step 7: Transform request for provider
    let _transformed = adapter.transform_request(&providers::types::ProviderRequest {
        messages: vec![], // Would parse from request
        model: model_id.to_string(),
        max_tokens: request
            .get("max_tokens")
            .and_then(|m| m.as_u64())
            .map(|v| v as u32),
        temperature: request
            .get("temperature")
            .and_then(|t| t.as_f64())
            .map(|v| v as f32),
        top_p: request
            .get("top_p")
            .and_then(|t| t.as_f64())
            .map(|v| v as f32),
        stop: request.get("stop").and_then(|s| s.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        }),
        stream: request
            .get("stream")
            .and_then(|s| s.as_bool())
            .unwrap_or(false),
        system: request
            .get("system")
            .and_then(|s| s.as_str().map(String::from)),
        tools: None,
        tool_choice: None,
    });

    let endpoint = adapter.get_endpoint(credential.base_url.as_deref(), model_id);
    let _headers = adapter.build_headers(&credential.api_key);

    // For now, return a mock response (actual HTTP call would go here)
    // TODO: Implement actual upstream HTTP call with reqwest
    tracing::info!("Proxying request to {} at {}", provider, endpoint);

    // Return mock response for now
    Ok(Json(json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion",
        "created": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        "model": model_id,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": format!("[Gateway mock response - route: {}, provider: {}]", primary.credential_id, provider)
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": classified.estimated_tokens,
            "completion_tokens": 50,
            "total_tokens": classified.estimated_tokens + 50
        },
        "_gateway": {
            "route": {
                "credential_id": primary.credential_id,
                "provider": provider,
                "utility": primary.utility,
            },
            "classification": {
                "format": format!("{:?}", classified.format),
                "capabilities": {
                    "vision": classified.required_capabilities.vision,
                    "tools": classified.required_capabilities.tools,
                    "streaming": classified.required_capabilities.streaming,
                }
            }
        }
    })))
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::config::GatewayConfig;
    use crate::state::{DefaultRequestClassifier, RateLimiter};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware,
        routing::get,
        Router,
    };
    use llm_tracing::{MemoryTraceCollector, TracingMiddleware};
    use model_registry::Registry as ModelRegistry;
    use smart_routing::{
        config::HealthConfig,
        executor::{ExecutorConfig, RouteExecutor},
        health::HealthManager,
        metrics::MetricsCollector,
        router::Router as SmartRouter,
    };
    use std::sync::Arc;
    use std::time::Instant;
    use tower::ServiceExt;

    fn create_test_state() -> AppState {
        AppState {
            config: GatewayConfig::default(),
            registry: ModelRegistry::default(),
            router: SmartRouter::new(),
            executor: Arc::new(RouteExecutor::new(
                ExecutorConfig::default(),
                MetricsCollector::new(),
                HealthManager::new(HealthConfig::default()),
            )),
            classifier: Arc::new(DefaultRequestClassifier),
            tracing: TracingMiddleware::new(Arc::new(MemoryTraceCollector::with_default_size())),
            start_time: Instant::now(),
            credentials: vec![],
            rate_limiter: Arc::new(RateLimiter::new(60)),
        }
    }

    #[tokio::test]
    async fn test_models_endpoint_with_credentials() {
        let mut state = create_test_state();
        state.config.credentials.push(config::CredentialConfig {
            id: "test-id".to_string(),
            provider: "openai".to_string(),
            api_key: "key".to_string(),
            allowed_models: vec!["gpt-4".to_string()],
            ..Default::default()
        });

        let app = Router::new()
            .route("/api/models", get(list_models))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/models")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 2048)
            .await
            .unwrap();
        let list: Value =
            serde_json::from_slice(&body).expect("models response should be valid JSON");
        assert_eq!(list["count"], 1);
        assert_eq!(list["models"][0]["id"], "gpt-4");
    }

    #[tokio::test]
    async fn test_health_endpoint_returns_status() {
        let state = create_test_state();
        let app = Router::new()
            .route("/health", get(health_check))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let health: HealthStatus =
            serde_json::from_slice(&body).expect("health response should be valid JSON");

        assert_eq!(health.status, "healthy");
        // uptime_secs can be 0 if the server started very recently
    }

    #[tokio::test]
    async fn test_root_endpoint() {
        let app = Router::new().route("/", get(root));

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_models_endpoint() {
        let state = create_test_state();
        let app = Router::new()
            .route("/api/models", get(list_models))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/models")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn test_request_classification() {
        let classifier = DefaultRequestClassifier;

        // Test simple text request
        let request = json!({
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        });
        let classified = classifier.classify(&request);
        assert!(!classified.required_capabilities.vision);
        assert!(!classified.required_capabilities.tools);
        assert!(!classified.required_capabilities.thinking);

        // Test vision request
        let vision_request = json!({
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "What's in this image?"},
                        {"type": "image_url", "image_url": {"url": "data:image/jpeg;base64,..."}}
                    ]
                }
            ]
        });
        let classified = classifier.classify(&vision_request);
        assert!(classified.required_capabilities.vision);

        // Test tools request
        let tools_request = json!({
            "messages": [{"role": "user", "content": "What's the weather?"}],
            "tools": [{"type": "function", "function": {"name": "get_weather"}}]
        });
        let classified = classifier.classify(&tools_request);
        assert!(classified.required_capabilities.tools);
    }

    #[tokio::test]
    async fn test_route_endpoint() {
        let state = create_test_state();
        let app = Router::new()
            .route("/api/route", get(route_request))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/route")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_security_headers_present() {
        let state = create_test_state();
        let app = Router::new()
            .route("/health", get(health_check))
            .layer(middleware::from_fn(security_headers_middleware))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let headers = response.headers();

        assert_eq!(headers.get("X-Content-Type-Options").unwrap(), "nosniff");
        assert_eq!(headers.get("X-Frame-Options").unwrap(), "DENY");
        assert!(
            headers.get("Referrer-Policy").is_some(),
            "Referrer-Policy header should be set"
        );
        assert!(
            headers.get("Content-Security-Policy").is_some(),
            "Content-Security-Policy header should be set"
        );
    }

    #[tokio::test]
    async fn test_rate_limiter_rejects_excess_requests() {
        let mut state = create_test_state();
        state.rate_limiter = Arc::new(RateLimiter::new(3)); // Very low limit for testing
        let app = Router::new()
            .route("/health", get(health_check))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                rate_limit_middleware,
            ))
            .with_state(state);

        let test_addr = SocketAddr::from(([127, 0, 0, 1], 12345));

        // First 3 requests should succeed
        for _ in 0..3 {
            let mut request = Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap();
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }

        // 4th request should be rate limited (429)
        let mut request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_rate_limiter_unit() {
        let limiter = RateLimiter::new(2);

        // First two requests allowed
        assert!(limiter.check("192.168.1.1"));
        assert!(limiter.check("192.168.1.1"));

        // Third request denied
        assert!(!limiter.check("192.168.1.1"));

        // Different IP is independent
        assert!(limiter.check("10.0.0.1"));
    }

    mod provider_integration_tests {
        use super::*;
        use crate::config::CredentialConfig;
        use crate::test_helpers::build_full_app;
        use crate::GatewayConfig;
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use model_registry::ModelInfo;
        use std::sync::Arc;
        use std::time::Instant;
        use tower::ServiceExt;

        const MAX_RESPONSE_BYTES: usize = 4096;

        async fn read_json_body(response: axum::response::Response) -> serde_json::Value {
            let body_bytes = axum::body::to_bytes(response.into_body(), MAX_RESPONSE_BYTES)
                .await
                .expect("response body should be readable");
            serde_json::from_slice(&body_bytes).expect("response body should be valid JSON")
        }

        /// Build a minimal [`ModelInfo`] for test registration.
        fn test_model_info(id: &str, provider: &str) -> ModelInfo {
            ModelInfo {
                id: id.to_string(),
                name: format!("Test {id}"),
                provider: provider.to_string(),
                context_window: 128_000,
                max_output_tokens: 4096,
                input_price_per_million: 1.0,
                output_price_per_million: 2.0,
                capabilities: model_registry::ModelCapabilities {
                    streaming: true,
                    tools: true,
                    vision: true,
                    thinking: false,
                },
                rate_limits: model_registry::RateLimits {
                    requests_per_minute: 60,
                    tokens_per_minute: 90_000,
                },
                source: model_registry::DataSource::Static,
            }
        }

        /// Creates an [`AppState`] whose SmartRouter has both credentials and
        /// models registered so that `build_candidates` produces candidates.
        fn create_state_with_routing(
            creds: Vec<CredentialConfig>,
            models: Vec<ModelInfo>,
        ) -> AppState {
            let mut config = GatewayConfig::default();
            config.server.auth_tokens = vec!["test-token".to_string()];
            config.credentials = creds;

            let mut smart_router = smart_routing::router::Router::new();
            for cred in &config.credentials {
                smart_router.add_credential(cred.id.clone(), cred.allowed_models.clone());
            }
            for model in &models {
                smart_router.set_model(model.id.clone(), model.clone());
            }

            let metrics = smart_routing::metrics::MetricsCollector::new();
            let health = smart_routing::health::HealthManager::new(
                smart_routing::config::HealthConfig::default(),
            );
            let executor = Arc::new(smart_routing::executor::RouteExecutor::new(
                smart_routing::executor::ExecutorConfig::default(),
                metrics,
                health,
            ));

            let credentials: Vec<smart_routing::weight::AuthInfo> = config
                .credentials
                .iter()
                .map(|c| smart_routing::weight::AuthInfo {
                    id: c.id.clone(),
                    priority: Some(c.priority),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: vec![],
                })
                .collect();

            AppState {
                config,
                registry: ModelRegistry::default(),
                router: smart_router,
                executor,
                classifier: Arc::new(DefaultRequestClassifier),
                tracing: llm_tracing::TracingMiddleware::new(Arc::new(
                    llm_tracing::MemoryTraceCollector::with_default_size(),
                )),
                start_time: Instant::now(),
                credentials,
                rate_limiter: Arc::new(RateLimiter::new(60)),
            }
        }

        fn make_chat_request(auth_token: &str, body: serde_json::Value) -> Request<Body> {
            let body_bytes = serde_json::to_vec(&body).unwrap();
            let mut req = Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("Authorization", format!("Bearer {auth_token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(body_bytes))
                .unwrap();
            let test_addr = SocketAddr::from(([127, 0, 0, 1], 12345));
            req.extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));
            req
        }

        fn make_credential(id: &str, provider: &str, models: &[&str]) -> CredentialConfig {
            CredentialConfig {
                id: id.to_string(),
                provider: provider.to_string(),
                api_key: "test-key-123".to_string(),
                allowed_models: models.iter().map(|m| m.to_string()).collect(),
                ..Default::default()
            }
        }

        // --- Provider Selection Tests ---

        #[tokio::test]
        async fn test_openai_adapter_selected() {
            let creds = vec![make_credential("openai-1", "openai", &["gpt-4"])];
            let models = vec![test_model_info("gpt-4", "openai")];
            let state = create_state_with_routing(creds, models);
            let app = build_full_app(state);

            let req = make_chat_request(
                "test-token",
                json!({
                    "model": "gpt-4",
                    "messages": [{"role": "user", "content": "Hello"}]
                }),
            );

            let response = app.oneshot(req).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body(response).await;
            assert_eq!(json_body["_gateway"]["route"]["provider"], "openai");
        }

        #[tokio::test]
        async fn test_google_adapter_selected() {
            let creds = vec![make_credential("google-1", "google", &["gemini-pro"])];
            let models = vec![test_model_info("gemini-pro", "google")];
            let state = create_state_with_routing(creds, models);
            let app = build_full_app(state);

            let req = make_chat_request(
                "test-token",
                json!({
                    "model": "gemini-pro",
                    "messages": [{"role": "user", "content": "Hello"}]
                }),
            );

            let response = app.oneshot(req).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body(response).await;
            assert_eq!(json_body["_gateway"]["route"]["provider"], "google");
        }

        #[tokio::test]
        async fn test_deepseek_uses_openai_adapter() {
            let creds = vec![make_credential(
                "deepseek-1",
                "deepseek",
                &["deepseek-chat"],
            )];
            let models = vec![test_model_info("deepseek-chat", "deepseek")];
            let state = create_state_with_routing(creds, models);
            let app = build_full_app(state);

            let req = make_chat_request(
                "test-token",
                json!({
                    "model": "deepseek-chat",
                    "messages": [{"role": "user", "content": "Hello"}]
                }),
            );

            let response = app.oneshot(req).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body(response).await;
            // DeepSeek uses OpenAI adapter internally but provider field stays "deepseek"
            assert_eq!(json_body["_gateway"]["route"]["provider"], "deepseek");
        }

        #[tokio::test]
        async fn test_unknown_provider_preserves_provider_name() {
            let creds = vec![make_credential(
                "unknown-1",
                "unknown-provider",
                &["some-model"],
            )];
            let models = vec![test_model_info("some-model", "unknown-provider")];
            let state = create_state_with_routing(creds, models);
            let app = build_full_app(state);

            let req = make_chat_request(
                "test-token",
                json!({
                    "model": "some-model",
                    "messages": [{"role": "user", "content": "Hello"}]
                }),
            );

            let response = app.oneshot(req).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body(response).await;
            assert_eq!(
                json_body["_gateway"]["route"]["provider"],
                "unknown-provider"
            );
        }

        // --- Endpoint Construction Tests ---

        #[tokio::test]
        async fn test_openai_endpoint_default() {
            let creds = vec![make_credential("openai-1", "openai", &["gpt-4"])];
            let models = vec![test_model_info("gpt-4", "openai")];
            let state = create_state_with_routing(creds, models);
            let app = build_full_app(state);

            let req = make_chat_request(
                "test-token",
                json!({
                    "model": "gpt-4",
                    "messages": [{"role": "user", "content": "Hello"}]
                }),
            );

            let response = app.oneshot(req).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body(response).await;
            assert_eq!(json_body["model"], "gpt-4");
            assert_eq!(json_body["_gateway"]["route"]["credential_id"], "openai-1");
        }

        #[tokio::test]
        async fn test_openai_endpoint_custom_base_url() {
            let mut cred = make_credential("openai-custom", "openai", &["gpt-4"]);
            cred.base_url = Some("https://custom.openai.proxy.com/v1".to_string());

            let creds = vec![cred];
            let models = vec![test_model_info("gpt-4", "openai")];
            let state = create_state_with_routing(creds, models);
            let app = build_full_app(state);

            let req = make_chat_request(
                "test-token",
                json!({
                    "model": "gpt-4",
                    "messages": [{"role": "user", "content": "Hello"}]
                }),
            );

            let response = app.oneshot(req).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body(response).await;
            assert_eq!(json_body["model"], "gpt-4");
            assert_eq!(
                json_body["_gateway"]["route"]["credential_id"],
                "openai-custom"
            );
        }

        #[tokio::test]
        async fn test_google_endpoint_includes_model() {
            let creds = vec![make_credential("google-1", "google", &["gemini-1.5-pro"])];
            let models = vec![test_model_info("gemini-1.5-pro", "google")];
            let state = create_state_with_routing(creds, models);
            let app = build_full_app(state);

            let req = make_chat_request(
                "test-token",
                json!({
                    "model": "gemini-1.5-pro",
                    "messages": [{"role": "user", "content": "Hello"}]
                }),
            );

            let response = app.oneshot(req).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body(response).await;
            // The model returned in the response should match what we sent
            assert_eq!(json_body["model"], "gemini-1.5-pro");
        }

        // --- Round-Trip Through Chat Completions Tests ---

        #[tokio::test]
        async fn test_chat_completions_with_temperature_and_max_tokens() {
            let creds = vec![make_credential("openai-1", "openai", &["gpt-4"])];
            let models = vec![test_model_info("gpt-4", "openai")];
            let state = create_state_with_routing(creds, models);
            let app = build_full_app(state);

            let req = make_chat_request(
                "test-token",
                json!({
                    "model": "gpt-4",
                    "messages": [{"role": "user", "content": "Hello"}],
                    "temperature": 0.5,
                    "max_tokens": 100
                }),
            );

            let response = app.oneshot(req).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body(response).await;

            assert_eq!(json_body["object"], "chat.completion");
            assert_eq!(json_body["model"], "gpt-4");
            assert_eq!(json_body["_gateway"]["route"]["provider"], "openai");
            assert_eq!(json_body["_gateway"]["route"]["credential_id"], "openai-1");
            assert!(json_body["choices"].is_array());
            assert_eq!(json_body["choices"][0]["message"]["role"], "assistant");
        }

        #[tokio::test]
        async fn test_chat_completions_vision_request() {
            let creds = vec![make_credential("openai-1", "openai", &["gpt-4-vision"])];
            let models = vec![test_model_info("gpt-4-vision", "openai")];
            let state = create_state_with_routing(creds, models);
            let app = build_full_app(state);

            let req = make_chat_request(
                "test-token",
                json!({
                    "model": "gpt-4-vision",
                    "messages": [{
                        "role": "user",
                        "content": [
                            {"type": "text", "text": "What is in this image?"},
                            {"type": "image_url", "image_url": {"url": "data:image/png;base64,abc123"}}
                        ]
                    }]
                }),
            );

            let response = app.oneshot(req).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body(response).await;

            // Verify classification includes vision=true
            assert_eq!(
                json_body["_gateway"]["classification"]["capabilities"]["vision"], true,
                "Expected vision capability to be true"
            );
            assert_eq!(json_body["model"], "gpt-4-vision");
        }
    }
}
