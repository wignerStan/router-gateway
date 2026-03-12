//! Gateway application builder and HTTP handlers.
//!
//! Provides [`build_app`] to construct the full Axum router from a
//! [`GatewayConfig`](config::GatewayConfig), plus all handler functions
//! and middleware used by the gateway.

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tower_http::trace::TraceLayer;

use crate::config::{self, GatewayConfig};
use crate::providers::{self, ProviderAdapter};

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

/// Default rate limit: requests per minute per client IP.
pub const DEFAULT_RATE_LIMIT: u64 = 60;

// ---------------------------------------------------------------------------
// Request classifier
// ---------------------------------------------------------------------------

/// Default request classifier implementation.
pub struct DefaultRequestClassifier;

impl RequestClassifier for DefaultRequestClassifier {
    fn classify(&self, request: &Value) -> smart_routing::classification::ClassifiedRequest {
        use smart_routing::classification::{
            ClassifiedRequest, QualityPreference, RequiredCapabilities,
        };

        let vision = ContentTypeDetector::detect_vision_required(request);
        let tools = ToolDetector::detect_tools_required(request);
        let streaming = StreamingExtractor::extract_streaming_preference(request);
        let thinking = false;

        let format = FormatDetector::detect(request);
        let estimated_tokens = TokenEstimator::estimate(request);
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

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

/// Application state shared across handlers.
pub struct AppState {
    pub config: GatewayConfig,
    pub registry: ModelRegistry,
    pub router: SmartRouter,
    pub executor: Arc<RouteExecutor>,
    pub classifier: Arc<DefaultRequestClassifier>,
    pub tracing: TracingMiddleware,
    pub start_time: Instant,
    pub credentials: Vec<AuthInfo>,
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
        }
    }
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Health status response.
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub uptime_secs: u64,
    pub credential_count: usize,
    pub healthy_count: usize,
    pub degraded_count: usize,
    pub unhealthy_count: usize,
}

/// Model information for API responses.
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub capabilities: Vec<String>,
    pub context_window: usize,
}

// ---------------------------------------------------------------------------
// Rate limiter
// ---------------------------------------------------------------------------

/// In-memory rate limiter tracking request counts per client IP
/// within a sliding one-minute window.
pub struct RateLimiter {
    buckets: Arc<Mutex<HashMap<String, (u64, Instant)>>>,
    max_requests: u64,
}

impl RateLimiter {
    pub fn new(max_requests: u64) -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
        }
    }

    /// Check whether a request from the given IP should be allowed.
    pub fn check(&self, ip: &str) -> bool {
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();

        let (count, window_start) = buckets.entry(ip.to_string()).or_insert((0, now));

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

    /// Remove expired rate-limit entries to bound memory growth.
    #[allow(dead_code)]
    pub fn prune(&self) {
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        buckets.retain(|_, (_, window_start)| now.duration_since(*window_start).as_secs() < 120);
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn root() -> Json<Value> {
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

/// Authentication middleware for protected routes.
pub async fn auth_middleware(
    State(state): State<AppState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> std::result::Result<axum::response::Response, (axum::http::StatusCode, Json<Value>)> {
    use axum::http::header::AUTHORIZATION;
    use axum::http::StatusCode;

    let is_development = std::env::var("GATEWAY_ENV")
        .map(|v| v.to_lowercase() == "development")
        .unwrap_or(false);

    if is_development && state.config.server.auth_tokens.is_empty() {
        tracing::warn!("Authentication skipped in development mode (no auth_tokens configured)");
        return Ok(next.run(req).await);
    }

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

    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    match auth_header {
        Some(header) => {
            if let Some(token) = header.strip_prefix("Bearer ") {
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
pub async fn security_headers_middleware(
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
        HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("1; mode=block"),
    );
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
    );
    headers.insert(
        HeaderName::from_static("strict-transport-security"),
        HeaderValue::from_static("max-age=63072000; includeSubDomains; preload"),
    );

    response
}

/// Rate limiting middleware. Uses the TCP peer address from `ConnectInfo<SocketAddr>`
/// to prevent IP spoofing via `X-Forwarded-For` headers.
pub async fn rate_limit_middleware(
    axum::extract::State(limiter): axum::extract::State<Arc<RateLimiter>>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> std::result::Result<axum::response::Response, (axum::http::StatusCode, Json<Value>)> {
    let client_ip = addr.ip().to_string();

    if !limiter.check(&client_ip) {
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

pub async fn health_check(State(state): State<AppState>) -> Json<HealthStatus> {
    let uptime = state.start_time.elapsed().as_secs();
    let credential_count = state.credentials.len();
    let healthy_count = credential_count;

    Json(HealthStatus {
        status: "healthy".to_string(),
        uptime_secs: uptime,
        credential_count,
        healthy_count,
        degraded_count: 0,
        unhealthy_count: 0,
    })
}

pub async fn list_models(State(state): State<AppState>) -> Json<Value> {
    let models: Vec<ModelInfo> = state
        .config
        .credentials
        .iter()
        .flat_map(|cred| {
            if cred.allowed_models.is_empty() {
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
                        capabilities: vec![],
                        context_window: 128_000,
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

pub async fn route_request(State(state): State<AppState>) -> Json<Value> {
    let sample_request = json!({
        "messages": [
            {
                "role": "user",
                "content": "Hello, how are you?"
            }
        ],
        "model": "unknown"
    });

    let classified = state.classifier.classify(&sample_request);
    let auths = state.credentials.clone();
    let session_id: Option<&str> = None;
    let route_plan = state.router.plan(&classified, auths, session_id).await;

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

/// POST /v1/chat/completions - Proxy endpoint for chat completion requests.
pub async fn chat_completions(
    State(state): State<AppState>,
    Json(request): Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    let classified = state.classifier.classify(&request);

    let model_id = request
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown");

    let auths = state.credentials.clone();
    let session_id = request.get("session_id").and_then(|s| s.as_str());
    let route_plan = state.router.plan(&classified, auths, session_id).await;

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

    let provider = &primary.provider;
    let adapter: Box<dyn ProviderAdapter> = match provider.as_str() {
        "google" => Box::new(providers::GoogleAdapter::new()),
        "gemini" => Box::new(providers::GoogleAdapter::new()),
        "xai" => Box::new(providers::OpenAIAdapter::new()),
        _ => Box::new(providers::OpenAIAdapter::new()),
    };

    let _transformed = adapter.transform_request(&providers::types::ProviderRequest {
        messages: vec![],
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

    tracing::info!("Proxying request to {} at {}", provider, endpoint);

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

// ---------------------------------------------------------------------------
// App builder
// ---------------------------------------------------------------------------

/// Build the complete Axum application router from a gateway configuration.
///
/// Constructs all state (registry, router, executor, classifier, tracing,
/// rate limiter), wires up routes and middleware layers, and returns a
/// ready-to-serve [`Router`].
pub fn build_app(config: GatewayConfig) -> Router {
    let registry = ModelRegistry::default();

    let mut smart_router = SmartRouter::new();
    for cred in &config.credentials {
        smart_router.add_credential(cred.id.clone(), cred.allowed_models.clone());
    }

    let metrics = MetricsCollector::new();
    let health = HealthManager::new(HealthConfig::default());
    let executor = Arc::new(RouteExecutor::new(
        ExecutorConfig::default(),
        metrics,
        health,
    ));

    let classifier = Arc::new(DefaultRequestClassifier);

    let collector = Arc::new(MemoryTraceCollector::with_default_size());
    let tracing_middleware = TracingMiddleware::new(collector);

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

    let rate_limiter = Arc::new(RateLimiter::new(DEFAULT_RATE_LIMIT));

    let state = AppState {
        config,
        registry,
        router: smart_router,
        executor,
        classifier,
        tracing: tracing_middleware.clone(),
        start_time: Instant::now(),
        credentials,
    };

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

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(axum::middleware::from_fn_with_state(
            rate_limiter,
            rate_limit_middleware,
        ))
        .layer(axum::middleware::from_fn(security_headers_middleware))
        .layer(axum::middleware::from_fn_with_state(
            tracing_middleware,
            llm_tracing::tracing_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
