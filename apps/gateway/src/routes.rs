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
/// Validates Bearer token against configured `auth_tokens`
/// Fails-closed by default (requires auth) unless `GATEWAY_ENV=development` is set
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
            .and_then(serde_json::Value::as_u64)
            .map(|v| v as u32),
        temperature: request
            .get("temperature")
            .and_then(serde_json::Value::as_f64)
            .map(|v| v as f32),
        top_p: request
            .get("top_p")
            .and_then(serde_json::Value::as_f64)
            .map(|v| v as f32),
        stop: request.get("stop").and_then(|s| s.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        }),
        stream: request
            .get("stream")
            .and_then(serde_json::Value::as_bool)
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
            .expect("system clock should be later than UNIX epoch")
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
    use model_registry::{
        DataSource, ModelCapabilities, ModelInfo as RegistryModelInfo, RateLimits,
        Registry as ModelRegistry,
    };
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

    const MAX_RESPONSE_BYTES: usize = 4096;

    const ERR_INVALID_REQUEST: &str = "invalid_request_error";
    const ERR_CONFIG_ERROR: &str = "config_error";
    const ERR_RATE_LIMIT: &str = "rate_limit_error";
    const ERR_NO_ROUTE: &str = "no_route_available";

    async fn read_json_body<T: serde::de::DeserializeOwned>(
        response: axum::response::Response,
    ) -> T {
        let body_bytes = axum::body::to_bytes(response.into_body(), MAX_RESPONSE_BYTES)
            .await
            .expect("response body should be readable");
        serde_json::from_slice(&body_bytes)
            .expect("test response body should deserialize as valid JSON")
    }

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
                    .expect("Axum request should be constructible"),
            )
            .await
            .expect("Router should handle request successfully");

        assert_eq!(response.status(), StatusCode::OK);
        let list: serde_json::Value = read_json_body(response).await;
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
                    .expect("Axum request should be constructible"),
            )
            .await
            .expect("Router should handle request successfully");

        assert_eq!(response.status(), StatusCode::OK);

        let health: HealthStatus = read_json_body(response).await;

        assert_eq!(health.status, "healthy");
        // uptime_secs can be 0 if the server started very recently
    }

    #[tokio::test]
    async fn test_root_endpoint() {
        let app = Router::new().route("/", get(root));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .expect("Axum request should be constructible"),
            )
            .await
            .expect("Router should handle request successfully");

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
                    .expect("Axum request should be constructible"),
            )
            .await
            .expect("Router should handle request successfully");

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
                    .expect("Axum request should be constructible"),
            )
            .await
            .expect("Router should handle request successfully");

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
                    .expect("Axum request should be constructible"),
            )
            .await
            .expect("Router should handle request successfully");

        let headers = response.headers();

        assert_eq!(
            headers
                .get("X-Content-Type-Options")
                .expect("X-Content-Type-Options header should be present"),
            "nosniff"
        );
        assert_eq!(
            headers
                .get("X-Frame-Options")
                .expect("X-Frame-Options header should be present"),
            "DENY"
        );
        assert_eq!(
            headers
                .get("Referrer-Policy")
                .expect("Referrer-Policy header should be present"),
            "strict-origin-when-cross-origin"
        );
        assert_eq!(
            headers
                .get("Content-Security-Policy")
                .expect("Content-Security-Policy header should be present"),
            "default-src 'none'; frame-ancestors 'none'"
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
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));
            let response = app
                .clone()
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);
        }

        // 4th request should be rate limited (429)
        let mut request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .expect("Axum request should be constructible");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));
        let response = app
            .oneshot(request)
            .await
            .expect("Router should handle request successfully");

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

    /// Register models in the router's candidate builder so that
    /// `build_candidates()` can produce route candidates from credentials.
    fn register_models_in_state(
        state: &mut AppState,
        caps: smart_routing::classification::RequiredCapabilities,
    ) {
        for cred in &state.config.credentials {
            for model_id in &cred.allowed_models {
                state.router.set_model(
                    model_id.clone(),
                    RegistryModelInfo {
                        id: model_id.clone(),
                        name: format!("Test Model {model_id}"),
                        provider: cred.provider.clone(),
                        context_window: 128_000,
                        max_output_tokens: 4096,
                        input_price_per_million: 1.0,
                        output_price_per_million: 2.0,
                        capabilities: ModelCapabilities {
                            streaming: caps.streaming,
                            tools: caps.tools,
                            vision: caps.vision,
                            thinking: caps.thinking,
                        },
                        rate_limits: RateLimits {
                            requests_per_minute: 60,
                            tokens_per_minute: 90_000,
                        },
                        source: DataSource::Static,
                    },
                );
            }
        }
    }

    fn register_models_in_state_all(state: &mut AppState) {
        register_models_in_state(
            state,
            smart_routing::classification::RequiredCapabilities {
                vision: true,
                tools: true,
                streaming: true,
                thinking: false,
            },
        );
    }

    // ----------------------------------------------------------------
    // Phase 1 integration tests using build_full_app()
    // ----------------------------------------------------------------

    use crate::test_helpers::{
        build_full_app, create_test_state as create_test_state_overrides, TestOverrides,
    };

    mod public_endpoints {
        use super::*;
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        #[tokio::test]
        async fn test_root_response_structure() {
            let state = create_test_state_overrides(TestOverrides::default());
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));

            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);

            let value = read_json_body::<serde_json::Value>(response).await;

            assert_eq!(value["name"], "Gateway API");
            assert_eq!(value["version"], "0.1.0");
            assert!(value["features"].is_array());
            assert!(value["endpoints"].is_object());
            assert_eq!(value["endpoints"]["health"], "/health");
            assert_eq!(value["endpoints"]["models"], "/api/models");
            assert_eq!(value["endpoints"]["route"], "/api/route");
        }

        #[tokio::test]
        async fn test_health_response_structure() {
            let state = create_test_state_overrides(TestOverrides::default());
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));

            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);

            let health: HealthStatus = read_json_body(response).await;

            assert_eq!(health.status, "healthy");
            // uptime_secs is u64, always >= 0
            assert_eq!(health.credential_count, 0);
            assert_eq!(health.healthy_count, 0);
            assert_eq!(health.degraded_count, 0);
            assert_eq!(health.unhealthy_count, 0);
        }

        #[tokio::test]
        async fn test_health_with_credentials() {
            let state = create_test_state_overrides(TestOverrides {
                credentials: vec![
                    config::CredentialConfig {
                        id: "cred-1".to_string(),
                        provider: "openai".to_string(),
                        api_key: "key-1".to_string(),
                        ..Default::default()
                    },
                    config::CredentialConfig {
                        id: "cred-2".to_string(),
                        provider: "anthropic".to_string(),
                        api_key: "key-2".to_string(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            });
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));

            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            let health: HealthStatus = read_json_body(response).await;

            assert_eq!(health.credential_count, 2);
            assert_eq!(health.healthy_count, 2);
        }

        #[tokio::test]
        async fn test_health_credential_counts() {
            let state = create_test_state_overrides(TestOverrides {
                credentials: vec![
                    config::CredentialConfig {
                        id: "a".to_string(),
                        provider: "openai".to_string(),
                        api_key: "k".to_string(),
                        ..Default::default()
                    },
                    config::CredentialConfig {
                        id: "b".to_string(),
                        provider: "openai".to_string(),
                        api_key: "k".to_string(),
                        ..Default::default()
                    },
                    config::CredentialConfig {
                        id: "c".to_string(),
                        provider: "google".to_string(),
                        api_key: "k".to_string(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            });
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));

            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            let health: HealthStatus = read_json_body(response).await;

            assert_eq!(health.credential_count, 3);
            assert_eq!(health.healthy_count, 3);
            assert_eq!(health.degraded_count, 0);
            assert_eq!(health.unhealthy_count, 0);
        }

        #[tokio::test]
        async fn test_public_endpoints_no_auth() {
            let state = create_test_state_overrides(TestOverrides::default());
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));
            let response = app
                .clone()
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);

            let mut request = Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));
            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);
        }
    }

    mod auth_middleware {
        use super::*;
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        #[tokio::test]
        async fn test_valid_bearer_token() {
            let state = create_test_state_overrides(TestOverrides::default());
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/api/models")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));

            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn test_invalid_token() {
            let state = create_test_state_overrides(TestOverrides::default());
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/api/models")
                .header("authorization", "Bearer wrong-token")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));

            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

            let value = read_json_body::<serde_json::Value>(response).await;
            assert_eq!(value["error"]["type"], ERR_INVALID_REQUEST);
            assert!(value["error"]["message"]
                .as_str()
                .expect("The 'message' field should be a string in the error response.")
                .contains("Invalid or expired API token"));
        }

        #[tokio::test]
        async fn test_missing_auth_header() {
            let state = create_test_state_overrides(TestOverrides::default());
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/api/models")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));

            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

            let value = read_json_body::<serde_json::Value>(response).await;
            assert!(value["error"]["message"]
                .as_str()
                .expect("JSON value should be a string")
                .contains("Missing Authorization header"));
        }

        #[tokio::test]
        async fn test_wrong_auth_scheme() {
            let state = create_test_state_overrides(TestOverrides::default());
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/api/models")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));

            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

            let value = read_json_body::<serde_json::Value>(response).await;
            assert!(value["error"]["message"]
                .as_str()
                .expect("JSON value should be a string")
                .contains("Invalid or expired API token"));
        }

        #[tokio::test]
        async fn test_no_auth_tokens_configured() {
            let state = create_test_state_overrides(TestOverrides {
                auth_tokens: vec![],
                ..Default::default()
            });
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/api/models")
                .header("authorization", "Bearer any-token")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));

            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::FORBIDDEN);

            let value = read_json_body::<serde_json::Value>(response).await;
            assert_eq!(value["error"]["type"], ERR_CONFIG_ERROR);
        }

        #[tokio::test]
        async fn test_second_token_valid() {
            let state = create_test_state_overrides(TestOverrides {
                auth_tokens: vec!["first-token".to_string(), "second-token".to_string()],
                ..Default::default()
            });
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/api/models")
                .header("authorization", "Bearer second-token")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));

            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);
        }
    }

    mod rate_limiting {
        use super::*;
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        async fn exhaust_rate_limit(app: &Router, test_addr: SocketAddr, limit: usize) {
            for _ in 0..limit {
                let mut request = Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("Axum request should be constructible");
                request
                    .extensions_mut()
                    .insert(axum::extract::ConnectInfo(test_addr));
                let response = app
                    .clone()
                    .oneshot(request)
                    .await
                    .expect("Router should handle request successfully");
                assert_eq!(response.status(), StatusCode::OK);
            }
        }

        #[tokio::test]
        async fn test_allows_requests_under_limit() {
            let state = create_test_state_overrides(TestOverrides {
                rate_limit: Some(5),
                ..Default::default()
            });
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            exhaust_rate_limit(&app, test_addr, 5).await;
        }

        #[tokio::test]
        async fn test_blocks_requests_over_limit() {
            let state = create_test_state_overrides(TestOverrides {
                rate_limit: Some(5),
                ..Default::default()
            });
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            exhaust_rate_limit(&app, test_addr, 5).await;

            let mut request = Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));
            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

            let value = read_json_body::<serde_json::Value>(response).await;
            assert_eq!(value["error"]["type"], ERR_RATE_LIMIT);
        }

        #[tokio::test]
        async fn test_independent_ip_buckets() {
            let state = create_test_state_overrides(TestOverrides {
                rate_limit: Some(1),
                ..Default::default()
            });
            let app = build_full_app(state);
            let addr_a = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));
            let addr_b = std::net::SocketAddr::from(([127, 0, 0, 2], 12345));

            let mut request = Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(addr_a));
            let response = app
                .clone()
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);

            let mut request = Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(addr_a));
            let response = app
                .clone()
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

            let mut request = Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(addr_b));
            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn test_rate_limit_response_body() {
            let state = create_test_state_overrides(TestOverrides {
                rate_limit: Some(1),
                ..Default::default()
            });
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            exhaust_rate_limit(&app, test_addr, 1).await;

            let mut request = Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));
            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

            let value = read_json_body::<serde_json::Value>(response).await;

            assert_eq!(value["error"]["type"], ERR_RATE_LIMIT);
            assert!(value["error"]["message"]
                .as_str()
                .expect("JSON value should be a string")
                .contains("Too many requests"));
        }

        #[tokio::test]
        async fn test_rate_limit_applies_to_public_endpoints() {
            let state = create_test_state_overrides(TestOverrides {
                rate_limit: Some(1),
                ..Default::default()
            });
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            exhaust_rate_limit(&app, test_addr, 1).await;

            let mut request = Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));
            let response = app
                .clone()
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

            let mut request = Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));
            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        }
    }

    mod protected_endpoints {
        use super::*;
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use serde_json::json;
        use tower::ServiceExt;

        #[tokio::test]
        async fn test_models_requires_auth() {
            let state = create_test_state_overrides(TestOverrides::default());
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/api/models")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));

            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }

        #[tokio::test]
        async fn test_route_requires_auth() {
            let state = create_test_state_overrides(TestOverrides::default());
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/api/route")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));

            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }

        #[tokio::test]
        async fn test_chat_completions_requires_auth() {
            let state = create_test_state_overrides(TestOverrides::default());
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let body = Body::from(
                serde_json::to_string(&json!({
                    "model": "gpt-4",
                    "messages": [{"role": "user", "content": "hello"}]
                }))
                .expect("Axum request should be constructible"),
            );
            let mut request = Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(body)
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));

            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }

        #[tokio::test]
        async fn test_models_with_auth_returns_list() {
            let state = create_test_state_overrides(TestOverrides {
                credentials: vec![config::CredentialConfig {
                    id: "test-cred".to_string(),
                    provider: "openai".to_string(),
                    api_key: "test-key".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            });
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/api/models")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));

            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);

            let value = read_json_body::<serde_json::Value>(response).await;

            assert!(value["models"].is_array());
            assert!(!value["models"]
                .as_array()
                .expect("Models list should be an array")
                .is_empty());
            assert_eq!(value["count"], 1);
        }
    }

    mod chat_completions {
        use super::*;
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use serde_json::json;
        use tower::ServiceExt;

        fn chat_request_body() -> Body {
            Body::from(
                serde_json::to_string(&json!({
                    "model": "gpt-4",
                    "messages": [{"role": "user", "content": "Hello"}]
                }))
                .expect("Axum request should be constructible"),
            )
        }

        fn make_chat_request(addr: std::net::SocketAddr) -> Request<Body> {
            let mut request = Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(chat_request_body())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(addr));
            request
        }

        #[tokio::test]
        async fn test_response_structure() {
            // build_full_app() populates the router via add_credential but does
            // not call set_model on the model registry, so the router finds no
            // candidates and chat_completions returns 503.  This test verifies
            // the correct error structure for that path.  Response-body tests for
            // the happy path live in the inline-router tests above.
            let state = create_test_state_overrides(TestOverrides {
                credentials: vec![config::CredentialConfig {
                    id: "test-cred".to_string(),
                    provider: "openai".to_string(),
                    api_key: "test-key".to_string(),
                    allowed_models: vec!["gpt-4".to_string()],
                    ..Default::default()
                }],
                ..Default::default()
            });
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let response = app
                .oneshot(make_chat_request(test_addr))
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

            let value = read_json_body::<serde_json::Value>(response).await;
            assert_eq!(value["error"]["type"], ERR_NO_ROUTE);
            assert!(value["error"]["message"]
                .as_str()
                .expect("JSON value should be a string")
                .contains("No suitable routes"));
        }

        #[tokio::test]
        async fn test_no_credentials_returns_503() {
            let state = create_test_state_overrides(TestOverrides {
                credentials: vec![],
                ..Default::default()
            });
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let response = app
                .oneshot(make_chat_request(test_addr))
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

            let value = read_json_body::<serde_json::Value>(response).await;

            assert_eq!(value["error"]["type"], ERR_NO_ROUTE);
        }

        #[tokio::test]
        async fn test_gateway_metadata() {
            let mut state = create_test_state_overrides(TestOverrides {
                credentials: vec![config::CredentialConfig {
                    id: "my-credential".to_string(),
                    provider: "openai".to_string(),
                    api_key: "key".to_string(),
                    allowed_models: vec!["gpt-4".to_string()],
                    ..Default::default()
                }],
                ..Default::default()
            });
            register_models_in_state_all(&mut state);
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let response = app
                .oneshot(make_chat_request(test_addr))
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);

            let value = read_json_body::<serde_json::Value>(response).await;

            let gw = &value["_gateway"];
            assert!(gw.is_object(), "Expected _gateway object in response");

            assert_eq!(gw["route"]["credential_id"], "my-credential");
            assert_eq!(gw["route"]["provider"], "openai");
            assert!(gw["classification"]["format"].is_string());

            let caps = &gw["classification"]["capabilities"];
            assert_eq!(caps["vision"], false);
            assert_eq!(caps["tools"], false);
            assert_eq!(caps["streaming"], false);
            // thinking is currently omitted by the chat_completions handler
            assert!(caps["thinking"].is_null());
        }

        #[tokio::test]
        async fn test_classification_in_response() {
            let mut state = create_test_state_overrides(TestOverrides {
                credentials: vec![config::CredentialConfig {
                    id: "test-cred".to_string(),
                    provider: "openai".to_string(),
                    api_key: "test-key".to_string(),
                    allowed_models: vec!["gpt-4".to_string()],
                    ..Default::default()
                }],
                ..Default::default()
            });
            register_models_in_state_all(&mut state);
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let response = app
                .oneshot(make_chat_request(test_addr))
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);

            let value = read_json_body::<serde_json::Value>(response).await;

            let caps = &value["_gateway"]["classification"]["capabilities"];
            assert_eq!(caps["vision"], false);
            assert_eq!(caps["tools"], false);
            assert_eq!(caps["streaming"], false);
            // thinking is currently omitted by the chat_completions handler
            assert!(caps["thinking"].is_null());
        }
    }

    mod middleware_composition {
        use super::*;
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        fn check_security_headers(headers: &axum::http::HeaderMap) {
            assert_eq!(
                headers
                    .get("X-Content-Type-Options")
                    .expect("X-Content-Type-Options header should be present"),
                "nosniff"
            );
            assert_eq!(
                headers
                    .get("X-Frame-Options")
                    .expect("X-Frame-Options header should be present"),
                "DENY"
            );
            assert_eq!(
                headers
                    .get("Referrer-Policy")
                    .expect("Referrer-Policy header should be present"),
                "strict-origin-when-cross-origin"
            );
            assert_eq!(
                headers
                    .get("Content-Security-Policy")
                    .expect("Content-Security-Policy header should be present"),
                "default-src 'none'; frame-ancestors 'none'"
            );
        }

        #[tokio::test]
        async fn test_security_headers_on_all_responses() {
            let state = create_test_state_overrides(TestOverrides::default());
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));
            let response = app
                .clone()
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            check_security_headers(response.headers());

            let mut request = Request::builder()
                .uri("/api/models")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));
            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            check_security_headers(response.headers());
        }

        #[tokio::test]
        async fn test_security_headers_on_error_responses() {
            let state = create_test_state_overrides(TestOverrides::default());
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/api/models")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));
            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            check_security_headers(response.headers());
        }

        #[tokio::test]
        async fn test_auth_before_handler() {
            let state = create_test_state_overrides(TestOverrides::default());
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/api/models")
                .header("authorization", "Bearer invalid-token")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));
            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");

            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            assert_ne!(response.status(), StatusCode::NOT_FOUND);
            assert_ne!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        }

        #[tokio::test]
        async fn test_rate_limit_before_auth() {
            let state = create_test_state_overrides(TestOverrides {
                rate_limit: Some(1),
                ..Default::default()
            });
            let app = build_full_app(state);
            let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

            let mut request = Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));
            let response = app
                .clone()
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);

            // No auth header — if auth ran before rate-limit, this would
            // return 401 UNAUTHORIZED, not 429 TOO_MANY_REQUESTS.
            let mut request = Request::builder()
                .uri("/api/models")
                .body(Body::empty())
                .expect("Axum request should be constructible");
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));
            let response = app
                .oneshot(request)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

            let value = read_json_body::<serde_json::Value>(response).await;
            assert_eq!(value["error"]["type"], ERR_RATE_LIMIT);
        }
    }

    // ----------------------------------------------------------------
    // Provider adapter integration tests
    // ----------------------------------------------------------------

    mod provider_integration_tests {
        use super::*;
        use crate::config::CredentialConfig;
        use crate::test_helpers::{
            build_full_app, create_test_state as create_test_state_overrides, TestOverrides,
        };

        fn make_credential(id: &str, provider: &str, models: &[&str]) -> CredentialConfig {
            CredentialConfig {
                id: id.to_string(),
                provider: provider.to_string(),
                api_key: "test-key-123".to_string(), // gitleaks:allow
                allowed_models: models
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect(),
                ..Default::default()
            }
        }

        fn make_chat_request(auth_token: &str, body: serde_json::Value) -> Request<Body> {
            let body_bytes = serde_json::to_vec(&body).expect("JSON body should be serializable");
            let mut req = Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("Authorization", format!("Bearer {auth_token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(body_bytes))
                .expect("Axum request should be constructible");
            let test_addr = SocketAddr::from(([127, 0, 0, 1], 12345));
            req.extensions_mut()
                .insert(axum::extract::ConnectInfo(test_addr));
            req
        }

        /// Build state with credentials and models registered for routing.
        fn create_routing_state(creds: Vec<CredentialConfig>) -> AppState {
            let mut state = create_test_state_overrides(TestOverrides {
                credentials: creds,
                ..Default::default()
            });
            register_models_in_state_all(&mut state);
            state
        }

        // --- Provider Selection Tests ---

        #[tokio::test]
        async fn test_openai_adapter_selected() {
            let creds = vec![make_credential("openai-1", "openai", &["gpt-4"])];
            let state = create_routing_state(creds);
            let app = build_full_app(state);

            let req = make_chat_request(
                "test-token",
                json!({
                    "model": "gpt-4",
                    "messages": [{"role": "user", "content": "Hello"}]
                }),
            );

            let response = app
                .oneshot(req)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body::<serde_json::Value>(response).await;
            assert_eq!(json_body["_gateway"]["route"]["provider"], "openai");
        }

        #[tokio::test]
        async fn test_google_adapter_selected() {
            let creds = vec![make_credential("google-1", "google", &["gemini-pro"])];
            let state = create_routing_state(creds);
            let app = build_full_app(state);

            let req = make_chat_request(
                "test-token",
                json!({
                    "model": "gemini-pro",
                    "messages": [{"role": "user", "content": "Hello"}]
                }),
            );

            let response = app
                .oneshot(req)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body::<serde_json::Value>(response).await;
            assert_eq!(json_body["_gateway"]["route"]["provider"], "google");
        }

        #[tokio::test]
        async fn test_deepseek_uses_openai_adapter() {
            let creds = vec![make_credential(
                "deepseek-1",
                "deepseek",
                &["deepseek-chat"],
            )];
            let state = create_routing_state(creds);
            let app = build_full_app(state);

            let req = make_chat_request(
                "test-token",
                json!({
                    "model": "deepseek-chat",
                    "messages": [{"role": "user", "content": "Hello"}]
                }),
            );

            let response = app
                .oneshot(req)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body::<serde_json::Value>(response).await;
            // DeepSeek uses OpenAI adapter internally but provider field stays "deepseek"
            assert_eq!(json_body["_gateway"]["route"]["provider"], "deepseek");
        }

        #[tokio::test]
        async fn test_unknown_provider_is_routed_successfully() {
            let creds = vec![make_credential(
                "unknown-1",
                "unknown-provider",
                &["some-model"],
            )];
            let state = create_routing_state(creds);
            let app = build_full_app(state);

            let req = make_chat_request(
                "test-token",
                json!({
                    "model": "some-model",
                    "messages": [{"role": "user", "content": "Hello"}]
                }),
            );

            let response = app
                .oneshot(req)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body::<serde_json::Value>(response).await;
            assert_eq!(
                json_body["_gateway"]["route"]["provider"],
                "unknown-provider"
            );
        }

        // --- Endpoint Construction Tests ---

        #[tokio::test]
        async fn test_openai_endpoint_default() {
            let creds = vec![make_credential("openai-1", "openai", &["gpt-4"])];
            let state = create_routing_state(creds);
            let app = build_full_app(state);

            let req = make_chat_request(
                "test-token",
                json!({
                    "model": "gpt-4",
                    "messages": [{"role": "user", "content": "Hello"}]
                }),
            );

            let response = app
                .oneshot(req)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body::<serde_json::Value>(response).await;
            assert_eq!(json_body["model"], "gpt-4");
            assert_eq!(json_body["_gateway"]["route"]["credential_id"], "openai-1");
        }

        #[tokio::test]
        async fn test_openai_endpoint_custom_base_url() {
            let mut cred = make_credential("openai-custom", "openai", &["gpt-4"]);
            cred.base_url = Some("https://custom.openai.proxy.com/v1".to_string());

            let creds = vec![cred];
            let state = create_routing_state(creds);
            let app = build_full_app(state);

            let req = make_chat_request(
                "test-token",
                json!({
                    "model": "gpt-4",
                    "messages": [{"role": "user", "content": "Hello"}]
                }),
            );

            let response = app
                .oneshot(req)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body::<serde_json::Value>(response).await;
            assert_eq!(json_body["model"], "gpt-4");
            assert_eq!(
                json_body["_gateway"]["route"]["credential_id"],
                "openai-custom"
            );
        }

        #[tokio::test]
        async fn test_google_endpoint_includes_model() {
            let creds = vec![make_credential("google-1", "google", &["gemini-1.5-pro"])];
            let state = create_routing_state(creds);
            let app = build_full_app(state);

            let req = make_chat_request(
                "test-token",
                json!({
                    "model": "gemini-1.5-pro",
                    "messages": [{"role": "user", "content": "Hello"}]
                }),
            );

            let response = app
                .oneshot(req)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body::<serde_json::Value>(response).await;
            assert_eq!(json_body["model"], "gemini-1.5-pro");
        }

        // --- Round-Trip Through Chat Completions Tests ---

        #[tokio::test]
        async fn test_chat_completions_with_temperature_and_max_tokens() {
            let creds = vec![make_credential("openai-1", "openai", &["gpt-4"])];
            let state = create_routing_state(creds);
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

            let response = app
                .oneshot(req)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body::<serde_json::Value>(response).await;

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
            let state = create_routing_state(creds);
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

            let response = app
                .oneshot(req)
                .await
                .expect("Router should handle request successfully");
            assert_eq!(response.status(), StatusCode::OK);

            let json_body = read_json_body::<serde_json::Value>(response).await;

            assert_eq!(
                json_body["_gateway"]["classification"]["capabilities"]["vision"], true,
                "Expected vision capability to be true"
            );
            assert_eq!(json_body["model"], "gpt-4-vision");
        }
    }
}
