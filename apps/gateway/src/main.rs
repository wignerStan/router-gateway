pub mod config;
pub mod providers;

use anyhow::Context;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Local package imports
use config::GatewayConfig;
use llm_tracing::{MemoryTraceCollector, TracingMiddleware};
use model_registry::Registry as ModelRegistry;
use smart_routing::{
    classification::{
        detection::ToolDetector, ContentTypeDetector, FormatDetector, RequestClassifier,
        StreamingExtractor, TokenEstimator,
    },
    config::HealthConfig,
    executor::{ExecutorConfig, RouteExecutor},
    health::HealthManager,
    metrics::MetricsCollector,
    router::Router as SmartRouter,
    weight::AuthInfo,
};

/// Default request classifier implementation
struct DefaultRequestClassifier;

impl RequestClassifier for DefaultRequestClassifier {
    fn classify(&self, request: &Value) -> smart_routing::classification::ClassifiedRequest {
        use smart_routing::classification::{
            ClassifiedRequest, QualityPreference, RequiredCapabilities,
        };

        // Detect required capabilities
        let vision = ContentTypeDetector::detect_vision_required(request);
        let tools = ToolDetector::detect_tools_required(request);
        let streaming = StreamingExtractor::extract_streaming_preference(request);
        let thinking = false; // Could be enhanced with reasoning detection

        // Detect format
        let format = FormatDetector::detect(request);

        // Estimate tokens
        let estimated_tokens = TokenEstimator::estimate(request);

        // Determine quality preference (could be enhanced with request analysis)
        let quality_preference = QualityPreference::Balanced;

        ClassifiedRequest {
            required_capabilities: RequiredCapabilities {
                vision,
                tools,
                streaming,
                thinking,
            },
            estimated_tokens,
            format,
            quality_preference,
        }
    }
}

/// Application state shared across handlers
struct AppState {
    /// Gateway configuration
    config: GatewayConfig,

    /// Model registry for model information
    registry: ModelRegistry,

    /// Smart router for route planning
    router: SmartRouter,

    /// Route executor for running requests
    executor: Arc<RouteExecutor>,

    /// Request classifier
    classifier: Arc<DefaultRequestClassifier>,

    /// Tracing middleware
    tracing: TracingMiddleware,

    /// Server start time for uptime tracking
    start_time: Instant,

    /// Credential information for routing
    credentials: Vec<AuthInfo>,

    /// Rate limiter for per-IP request throttling
    rate_limiter: Arc<RateLimiter>,
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            registry: self.registry.clone(),
            router: self.router.clone(),
            executor: Arc::clone(&self.executor),
            classifier: Arc::clone(&self.classifier),
            tracing: self.tracing.clone(),
            start_time: self.start_time,
            credentials: self.credentials.clone(),
            rate_limiter: Arc::clone(&self.rate_limiter),
        }
    }
}

/// Health status response
#[derive(Debug, Serialize, Deserialize)]
struct HealthStatus {
    status: String,
    uptime_secs: u64,
    credential_count: usize,
    healthy_count: usize,
    degraded_count: usize,
    unhealthy_count: usize,
}

/// Model information for API response
#[derive(Debug, Serialize, Deserialize)]
struct ModelInfo {
    id: String,
    provider: String,
    capabilities: Vec<String>,
    context_window: usize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "gateway=debug,tower_http=debug,axum=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = load_config()?;
    tracing::info!(
        "Loaded configuration with {} credentials",
        config.credentials.len()
    );

    // Warn if authentication is disabled
    if !config.is_auth_enabled() {
        tracing::warn!(
            "WARNING: No auth_tokens configured — authentication is DISABLED. \
             This is not recommended for production deployments."
        );
    }

    // Initialize model registry
    let registry = ModelRegistry::default();
    tracing::info!("Model registry initialized");

    // Initialize smart router and populate from config
    let mut smart_router = SmartRouter::new();

    // Populate router with credentials and models from config
    for cred in &config.credentials {
        // Add credential with its allowed models to the router
        smart_router.add_credential(cred.id.clone(), cred.allowed_models.clone());
    }

    tracing::info!(
        "Smart router initialized with {} credentials",
        config.credentials.len()
    );

    // Initialize metrics and health for executor
    let metrics = MetricsCollector::new();
    let health = HealthManager::new(HealthConfig::default());

    // Initialize route executor
    let executor_config = ExecutorConfig::default();
    let executor = Arc::new(RouteExecutor::new(executor_config, metrics, health));
    tracing::info!("Route executor initialized");

    // Initialize request classifier
    let classifier = Arc::new(DefaultRequestClassifier);

    // Initialize tracing middleware
    let collector = Arc::new(MemoryTraceCollector::with_default_size());
    let tracing_middleware = TracingMiddleware::new(collector);
    tracing::info!("LLM tracing initialized");

    // Convert credentials to AuthInfo for routing
    let credentials: Vec<AuthInfo> = config
        .credentials
        .iter()
        .map(|c| AuthInfo {
            id: c.id.clone(),
            priority: Some(c.priority),
            quota_exceeded: false,
            unavailable: false,
            model_states: vec![],
        })
        .collect();

    // Initialize rate limiter
    let rate_limiter = Arc::new(RateLimiter::new(DEFAULT_RATE_LIMIT));
    tracing::info!(
        "Rate limiter initialized: {} requests per minute per IP",
        DEFAULT_RATE_LIMIT
    );

    let state = AppState {
        config,
        registry,
        router: smart_router,
        executor,
        classifier,
        tracing: tracing_middleware.clone(),
        start_time: Instant::now(),
        credentials,
        rate_limiter,
    };

    // Get host and port from config before moving state
    let port = state.config.server.port;
    let host = &state.config.server.host;
    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .context("Invalid host/port configuration")?;
    tracing::info!("Gateway listening on {}", addr);

    // Build our application with routes
    // Public routes (no authentication required)
    let public_routes = Router::new()
        .route("/", get(root))
        .route("/health", get(health_check));

    // Protected routes (require authentication if auth_tokens configured)
    let protected_routes = Router::new()
        .route("/api/models", get(list_models))
        .route("/api/route", get(route_request))
        .route("/v1/chat/completions", post(chat_completions))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .layer(axum::middleware::from_fn(security_headers_middleware))
        .layer(axum::middleware::from_fn_with_state(
            tracing_middleware,
            llm_tracing::tracing_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Load configuration from file or environment
fn load_config() -> anyhow::Result<GatewayConfig> {
    // Try to load from GATEWAY_CONFIG env var or default paths
    let config_path = std::env::var("GATEWAY_CONFIG").ok().or_else(|| {
        // Check for common config file locations
        for path in ["./gateway.yaml", "./config/gateway.yaml", "./gateway.yml"] {
            if std::path::Path::new(path).exists() {
                return Some(path.to_string());
            }
        }
        None
    });

    match config_path {
        Some(path) => {
            tracing::info!("Loading configuration from {}", path);
            match GatewayConfig::from_file(&path) {
                Ok(config) => Ok(config),
                Err(e) => {
                    anyhow::bail!(
                        "Failed to load config from {}: {}. Please fix the configuration file.",
                        path,
                        e
                    );
                },
            }
        },
        None => {
            tracing::info!("No configuration file found, using defaults");
            Ok(GatewayConfig::default())
        },
    }
}

async fn root() -> Json<Value> {
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
async fn auth_middleware(
    State(state): State<AppState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
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
                if state
                    .config
                    .server
                    .auth_tokens
                    .iter()
                    .any(|t| config::constant_time_token_eq(t, token))
                {
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
async fn security_headers_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::http::header::HeaderName;
    use axum::http::HeaderValue;

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

/// Default rate limit: requests per minute per client IP.
const DEFAULT_RATE_LIMIT: u64 = 60;

/// In-memory rate limiter tracking request counts per client IP
/// within a sliding one-minute window.
struct RateLimiter {
    /// IP address -> (request count, window start time)
    buckets: Arc<Mutex<HashMap<String, (u64, Instant)>>>,
    max_requests: u64,
}

impl RateLimiter {
    fn new(max_requests: u64) -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
        }
    }

    /// Check whether a request from the given IP should be allowed.
    /// Returns `true` if under the limit, `false` if rate limited.
    /// Also prunes stale entries (inactive for >2 minutes) to prevent unbounded growth.
    fn check(&self, ip: &str) -> bool {
        let mut buckets = self.buckets.lock().unwrap();
        let now = Instant::now();

        // Prune stale entries to prevent unbounded memory growth
        buckets.retain(|_, (_, window_start)| now.duration_since(*window_start).as_secs() < 120);

        let (count, window_start) = buckets.entry(ip.to_string()).or_insert((0, now));

        // Reset window if more than 60 seconds have passed
        if now.duration_since(*window_start).as_secs() >= 60 {
            *count = 0;
            *window_start = now;
        }

        if *count >= self.max_requests {
            return false;
        }

        *count += 1;
        true
    }
}

/// Rate limiting middleware. Extracts client IP from X-Forwarded-For or
/// X-Real-IP headers only when `trust_proxy_headers` is enabled (gateway behind
/// a trusted reverse proxy). Otherwise, all requests share a single bucket to
/// prevent header-spoofing bypasses.
async fn rate_limit_middleware(
    axum::extract::State(state): axum::extract::State<AppState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> std::result::Result<axum::response::Response, (axum::http::StatusCode, Json<Value>)> {
    let client_ip = if state.config.server.trust_proxy_headers {
        req.headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or_else(|| req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()))
            .unwrap_or("unknown")
    } else {
        "shared"
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

async fn health_check(State(state): State<AppState>) -> Json<HealthStatus> {
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

async fn list_models(State(state): State<AppState>) -> Json<Value> {
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

async fn route_request(State(state): State<AppState>) -> Json<Value> {
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
async fn chat_completions(
    State(state): State<AppState>,
    Json(request): Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    use providers::{AnthropicAdapter, OpenAIAdapter, ProviderAdapter};

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
    let credential = state
        .config
        .credentials
        .iter()
        .find(|c| c.id == primary.credential_id);

    let (api_key, base_url) = match credential {
        Some(cred) => (cred.api_key.clone(), cred.base_url.clone()),
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
        "anthropic" => Box::new(AnthropicAdapter::new()),
        "openai" | "azure-openai" => Box::new(OpenAIAdapter::new()),
        _ => Box::new(OpenAIAdapter::new()), // Default to OpenAI format
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

    let endpoint = adapter.get_endpoint(base_url.as_deref(), model_id);
    let _headers = adapter.build_headers(&api_key);

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
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
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
        let list: Value = serde_json::from_slice(&body).unwrap();
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
        let health: HealthStatus = serde_json::from_slice(&body).unwrap();

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
            .layer(axum::middleware::from_fn(security_headers_middleware))
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
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                rate_limit_middleware,
            ))
            .with_state(state);

        // First 3 requests should succeed
        for _ in 0..3 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/health")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }

        // 4th request should be rate limited (429)
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

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
}
