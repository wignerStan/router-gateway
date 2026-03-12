use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::State,
    middleware::{self, Next},
    routing::{get, post},
    Json, Router,
};
use gateway::config::{constant_time_token_matches, CredentialConfig, GatewayConfig};
use reqwest::Client;
use serde_json::{json, Value};

/// Real HTTP test server bound to a random localhost port.
pub struct GatewayTestServer {
    pub url: String,
    pub client: Client,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl GatewayTestServer {
    pub fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
    }
}

/// Internal application state for the test server.
#[derive(Clone)]
struct TestState {
    config: GatewayConfig,
    start_time: Instant,
}

/// Standalone rate limiter (mirrors production `RateLimiter`).
struct TestRateLimiter {
    buckets: Arc<std::sync::Mutex<HashMap<String, (u64, Instant)>>>,
    max_requests: u64,
}

impl TestRateLimiter {
    fn check(&self, ip: &str) -> bool {
        let mut buckets = self.buckets.lock().unwrap();
        let now = Instant::now();
        let (count, window) = buckets.entry(ip.to_string()).or_insert((0, now));
        if now.duration_since(*window).as_secs() >= 60 {
            *count = 0;
            *window = now;
        }
        if *count >= self.max_requests {
            return false;
        }
        *count += 1;
        true
    }
}

/// Spawn a gateway test server with default rate limit (100 req/min).
pub async fn spawn_gateway(config: GatewayConfig) -> GatewayTestServer {
    spawn_gateway_with_rate_limit(config, 100).await
}

/// Spawn a gateway test server with a custom per-IP rate limit.
pub async fn spawn_gateway_with_rate_limit(
    config: GatewayConfig,
    rate_limit: u64,
) -> GatewayTestServer {
    let state = TestState {
        config,
        start_time: Instant::now(),
    };

    let rate_limiter = Arc::new(TestRateLimiter {
        buckets: Arc::new(std::sync::Mutex::new(HashMap::new())),
        max_requests: rate_limit,
    });

    let public = Router::new()
        .route("/", get(root_handler))
        .route("/health", get(health_handler));

    let protected = Router::new()
        .route("/api/models", get(list_models_handler))
        .route("/api/route", get(route_handler))
        .route("/v1/chat/completions", post(chat_completions_handler))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let app = Router::new()
        .merge(public)
        .merge(protected)
        .layer(middleware::from_fn(
            move |req: axum::http::Request<axum::body::Body>, next: middleware::Next| {
                let limiter = Arc::clone(&rate_limiter);
                async move {
                    let ip = req
                        .headers()
                        .get("x-forwarded-for")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.split(',').next())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .or_else(|| req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()))
                        .unwrap_or("unknown");
                    if !limiter.check(ip) {
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
            },
        ))
        .layer(middleware::from_fn(security_headers_middleware))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind test server");
    let port = listener.local_addr().unwrap().port();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        tokio::select! {
            r = axum::serve(listener, app) => { r.ok(); }
            _ = shutdown_rx => {}
        }
    });

    GatewayTestServer {
        url: format!("http://127.0.0.1:{port}"),
        client: Client::new(),
        shutdown_tx,
    }
}

// ---------------------------------------------------------------------------
// Config helpers
// ---------------------------------------------------------------------------

/// Config with no auth tokens and no credentials.
#[allow(dead_code)]
pub fn test_config_no_auth() -> GatewayConfig {
    GatewayConfig::default()
}

/// Config with auth enabled but no credentials.
pub fn test_config_with_auth(token: &str) -> GatewayConfig {
    let mut config = GatewayConfig::default();
    config.server.auth_tokens = vec![token.to_string()];
    config
}

/// Sample credentials spanning multiple providers.
#[allow(dead_code)]
pub fn test_credentials() -> Vec<CredentialConfig> {
    vec![
        CredentialConfig {
            id: "openai-primary".into(),
            provider: "openai".into(),
            api_key: "sk-test".into(),
            allowed_models: vec!["gpt-4".into(), "gpt-3.5-turbo".into()],
            priority: 10,
            ..Default::default()
        },
        CredentialConfig {
            id: "openai-backup".into(),
            provider: "openai".into(),
            api_key: "sk-test-backup".into(),
            allowed_models: vec!["gpt-4".into()],
            priority: 5,
            ..Default::default()
        },
        CredentialConfig {
            id: "anthropic-primary".into(),
            provider: "anthropic".into(),
            api_key: "sk-ant-test".into(),
            allowed_models: vec!["claude-3-opus".into()],
            priority: 8,
            ..Default::default()
        },
    ]
}

// ---------------------------------------------------------------------------
// Handlers (mirror production behavior from main.rs)
// ---------------------------------------------------------------------------

async fn root_handler() -> Json<Value> {
    Json(json!({
        "name": "Gateway API",
        "version": "0.1.0"
    }))
}

async fn health_handler(State(state): State<TestState>) -> Json<Value> {
    let count = state.config.credentials.len();
    Json(json!({
        "status": "healthy",
        "uptime_secs": state.start_time.elapsed().as_secs(),
        "credential_count": count,
        "healthy_count": count,
        "degraded_count": 0,
        "unhealthy_count": 0
    }))
}

async fn list_models_handler(State(state): State<TestState>) -> Json<Value> {
    let models: Vec<Value> = state
        .config
        .credentials
        .iter()
        .flat_map(|c| {
            if c.allowed_models.is_empty() {
                vec![json!({
                    "id": format!("{}:*", c.provider),
                    "provider": c.provider
                })]
            } else {
                c.allowed_models
                    .iter()
                    .map(|m| json!({ "id": m, "provider": c.provider }))
                    .collect()
            }
        })
        .collect();
    let count = models.len();
    Json(json!({
        "models": models,
        "count": count,
        "message": if count > 0 { "Models loaded from configuration" } else { "No models configured" }
    }))
}

async fn route_handler(State(state): State<TestState>) -> Json<Value> {
    let has_creds = !state.config.credentials.is_empty();
    let primary = state.config.credentials.first().map(|c| {
        json!({
            "credential_id": c.id,
            "provider": c.provider
        })
    });
    Json(json!({
        "route_plan": {
            "primary": primary,
            "total_candidates": state.config.credentials.len()
        },
        "message": if has_creds { "Route planned successfully" } else { "No suitable routes found" }
    }))
}

async fn chat_completions_handler(
    State(state): State<TestState>,
    Json(req): Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    let model = req
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown");

    let cred = state
        .config
        .credentials
        .iter()
        .find(|c| c.allowed_models.is_empty() || c.allowed_models.iter().any(|m| m == model));

    let (cred_id, provider) = match cred {
        Some(c) => (c.id.clone(), c.provider.clone()),
        None if state.config.credentials.is_empty() => {
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
        None => {
            let c = &state.config.credentials[0];
            (c.id.clone(), c.provider.clone())
        },
    };

    Ok(Json(json!({
        "id": format!("chatcmpl-{}", nanoid()),
        "object": "chat.completion",
        "created": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        "model": model,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": format!("[Gateway mock response - route: {}, provider: {}]", cred_id, provider)
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 50,
            "total_tokens": 60
        },
        "_gateway": {
            "route": {
                "credential_id": cred_id,
                "provider": provider
            }
        }
    })))
}

fn nanoid() -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{n:x}")
}

// ---------------------------------------------------------------------------
// Middleware (mirrors production behavior from main.rs)
// ---------------------------------------------------------------------------

async fn auth_middleware(
    State(state): State<TestState>,
    req: axum::extract::Request,
    next: Next,
) -> std::result::Result<axum::response::Response, (axum::http::StatusCode, Json<Value>)> {
    if state.config.server.auth_tokens.is_empty() {
        return Err((
            axum::http::StatusCode::FORBIDDEN,
            Json(json!({
                "error": {
                    "type": "config_error",
                    "message": "No authentication tokens configured"
                }
            })),
        ));
    }

    let header = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok());

    match header {
        Some(h) => {
            if let Some(token) = h.strip_prefix("Bearer ") {
                if constant_time_token_matches(token, &state.config.server.auth_tokens) {
                    return Ok(next.run(req).await);
                }
            }
            Err((
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": {
                        "type": "invalid_request_error",
                        "message": "Invalid or expired API token"
                    }
                })),
            ))
        },
        None => Err((
            axum::http::StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": {
                    "type": "invalid_request_error",
                    "message": "Missing Authorization header. Use: Authorization: Bearer <token>"
                }
            })),
        )),
    }
}

async fn security_headers_middleware(
    req: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    use axum::http::header::HeaderName;
    use axum::http::HeaderValue;

    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    h.insert(
        HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("1; mode=block"),
    );
    h.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    resp
}
