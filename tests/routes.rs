#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::needless_pass_by_value,
    clippy::panic
)]
mod common;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    routing::get,
};
use gateway::config::{CredentialConfig, GatewayConfig};
use gateway::registry::{
    DataSource, ModelCapabilities, ModelInfo as RegistryModelInfo, RateLimits,
    Registry as ModelRegistry,
};
use gateway::routes::{health_check, list_models, root, route_request};
use gateway::routing::classification::RequestClassifier;
use gateway::routing::{
    config::HealthConfig,
    executor::{ExecutorConfig, RouteExecutor},
    health::HealthManager,
    metrics::MetricsCollector,
    router::Router as SmartRouter,
};
use gateway::state::{AppState, DefaultRequestClassifier, HealthStatus, RateLimiter};
use gateway::tracing::{MemoryTraceCollector, TracingMiddleware};
use gateway::{build_app_router, build_app_state};
use serde_json::json;
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

// --- Test helpers (moved from lib.rs test_helpers) ---

struct TestOverrides {
    auth_tokens: Vec<String>,
    credentials: Vec<CredentialConfig>,
    rate_limit: Option<u64>,
}

impl Default for TestOverrides {
    fn default() -> Self {
        Self {
            auth_tokens: vec!["test-token".to_string()],
            credentials: vec![],
            rate_limit: None,
        }
    }
}

fn create_test_state_overrides(overrides: TestOverrides) -> AppState {
    let mut config = GatewayConfig::default();
    config.server.auth_tokens = overrides.auth_tokens;
    config.credentials = overrides.credentials;
    build_app_state(config, overrides.rate_limit)
}

fn build_full_app(state: AppState) -> Router {
    build_app_router(state)
}

// --- Helper: register models in state ---

fn register_models_in_state(
    state: &mut AppState,
    caps: &gateway::routing::classification::RequiredCapabilities,
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
        &gateway::routing::classification::RequiredCapabilities {
            vision: true,
            tools: true,
            streaming: true,
            thinking: false,
        },
    );
}

// ===================================================================
// Tests
// ===================================================================

#[tokio::test]
async fn test_models_endpoint_with_credentials() {
    let mut state = create_test_state();
    state.config.credentials.push(CredentialConfig {
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
                .expect("time went backwards"),
        )
        .await
        .expect("time went backwards");

    assert_eq!(response.status(), StatusCode::OK);
    let list: serde_json::Value = common::read_json(response).await;
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
                .expect("time went backwards"),
        )
        .await
        .expect("time went backwards");

    assert_eq!(response.status(), StatusCode::OK);

    let health: HealthStatus = common::read_json(response).await;

    assert_eq!(health.status, "healthy");
}

#[tokio::test]
async fn test_root_endpoint() {
    let app = Router::new().route("/", get(root));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("time went backwards"),
        )
        .await
        .expect("time went backwards");

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
                .expect("time went backwards"),
        )
        .await
        .expect("time went backwards");

    assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn test_request_classification() {
    let classifier = DefaultRequestClassifier;

    let request = json!({
        "messages": [
            {"role": "user", "content": "Hello"}
        ]
    });
    let classified = classifier.classify(&request);
    assert!(!classified.required_capabilities.vision);
    assert!(!classified.required_capabilities.tools);
    assert!(!classified.required_capabilities.thinking);

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
                .expect("time went backwards"),
        )
        .await
        .expect("time went backwards");

    assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn test_rate_limiter_unit() {
    let limiter = RateLimiter::new(2);

    assert!(limiter.check("192.168.1.1"));
    assert!(limiter.check("192.168.1.1"));

    assert!(!limiter.check("192.168.1.1"));

    assert!(limiter.check("10.0.0.1"));
}

// ===================================================================
// Public endpoints
// ===================================================================

mod public_endpoints {
    use super::*;

    #[tokio::test]
    async fn test_root_response_structure() {
        let state = create_test_state_overrides(TestOverrides::default());
        let app = build_full_app(state);
        let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

        let mut request = Request::builder()
            .uri("/")
            .body(Body::empty())
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));

        let response = app.oneshot(request).await.expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);

        let value = common::read_json::<serde_json::Value>(response).await;
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
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));

        let response = app.oneshot(request).await.expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);

        let health: HealthStatus = common::read_json(response).await;

        assert_eq!(health.status, "healthy");
        assert_eq!(health.credential_count, 0);
        assert_eq!(health.healthy_count, 0);
        assert_eq!(health.degraded_count, 0);
        assert_eq!(health.unhealthy_count, 0);
    }

    #[tokio::test]
    async fn test_health_with_credentials() {
        let state = create_test_state_overrides(TestOverrides {
            credentials: vec![
                CredentialConfig {
                    id: "cred-1".to_string(),
                    provider: "openai".to_string(),
                    api_key: "key-1".to_string(),
                    ..Default::default()
                },
                CredentialConfig {
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
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));

        let response = app.oneshot(request).await.expect("time went backwards");
        let health: HealthStatus = common::read_json(response).await;

        assert_eq!(health.credential_count, 2);
        assert_eq!(health.healthy_count, 2);
    }

    #[tokio::test]
    async fn test_health_credential_counts() {
        let state = create_test_state_overrides(TestOverrides {
            credentials: vec![
                CredentialConfig {
                    id: "a".to_string(),
                    provider: "openai".to_string(),
                    api_key: "k".to_string(),
                    ..Default::default()
                },
                CredentialConfig {
                    id: "b".to_string(),
                    provider: "openai".to_string(),
                    api_key: "k".to_string(),
                    ..Default::default()
                },
                CredentialConfig {
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
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));

        let response = app.oneshot(request).await.expect("time went backwards");
        let health: HealthStatus = common::read_json(response).await;

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
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));
        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);

        let mut request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));
        let response = app.oneshot(request).await.expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);
    }
}

// ===================================================================
// Protected endpoints
// ===================================================================

mod protected_endpoints {
    use super::*;

    #[tokio::test]
    async fn test_models_requires_auth() {
        let state = create_test_state_overrides(TestOverrides::default());
        let app = build_full_app(state);
        let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

        let mut request = Request::builder()
            .uri("/api/models")
            .body(Body::empty())
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));

        let response = app.oneshot(request).await.expect("time went backwards");
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
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));

        let response = app.oneshot(request).await.expect("time went backwards");
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
            .expect("time went backwards"),
        );
        let mut request = Request::builder()
            .method("POST")
            .uri("/v1/chat/completions")
            .header("content-type", "application/json")
            .body(body)
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));

        let response = app.oneshot(request).await.expect("time went backwards");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_models_with_auth_returns_list() {
        let state = create_test_state_overrides(TestOverrides {
            credentials: vec![CredentialConfig {
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
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));

        let response = app.oneshot(request).await.expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);

        let value = common::read_json::<serde_json::Value>(response).await;

        assert!(value["models"].is_array());
        assert!(
            !value["models"]
                .as_array()
                .expect("time went backwards")
                .is_empty()
        );
        assert_eq!(value["count"], 1);
    }
}

// ===================================================================
// Chat completions
// ===================================================================

mod chat_completions {
    use super::*;

    fn chat_request_body() -> Body {
        Body::from(
            serde_json::to_string(&json!({
                "model": "gpt-4",
                "messages": [{"role": "user", "content": "Hello"}]
            }))
            .expect("time went backwards"),
        )
    }

    fn make_chat_request(addr: std::net::SocketAddr) -> Request<Body> {
        let mut request = Request::builder()
            .method("POST")
            .uri("/v1/chat/completions")
            .header("content-type", "application/json")
            .header("authorization", "Bearer test-token")
            .body(chat_request_body())
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(addr));
        request
    }

    #[tokio::test]
    async fn test_response_structure() {
        let state = create_test_state_overrides(TestOverrides {
            credentials: vec![CredentialConfig {
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
            .expect("time went backwards");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let value = common::read_json::<serde_json::Value>(response).await;
        assert_eq!(value["error"]["type"], common::ERR_NO_ROUTE);
        assert!(
            value["error"]["message"]
                .as_str()
                .expect("time went backwards")
                .contains("No suitable routes")
        );
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
            .expect("time went backwards");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let value = common::read_json::<serde_json::Value>(response).await;

        assert_eq!(value["error"]["type"], common::ERR_NO_ROUTE);
    }

    #[tokio::test]
    async fn test_gateway_metadata() {
        let mut state = create_test_state_overrides(TestOverrides {
            credentials: vec![CredentialConfig {
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
            .expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);

        let value = common::read_json::<serde_json::Value>(response).await;

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
            credentials: vec![CredentialConfig {
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
            .expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);

        let value = common::read_json::<serde_json::Value>(response).await;

        let caps = &value["_gateway"]["classification"]["capabilities"];
        assert_eq!(caps["vision"], false);
        assert_eq!(caps["tools"], false);
        assert_eq!(caps["streaming"], false);
        // thinking is currently omitted by the chat_completions handler
        assert!(caps["thinking"].is_null());
    }
}

// ===================================================================
// Middleware composition
// ===================================================================

mod middleware_composition {
    use super::*;

    fn check_security_headers(headers: &axum::http::HeaderMap) {
        assert_eq!(
            headers
                .get("X-Content-Type-Options")
                .expect("time went backwards"),
            "nosniff"
        );
        assert_eq!(
            headers.get("X-Frame-Options").expect("time went backwards"),
            "DENY"
        );
        assert_eq!(
            headers.get("Referrer-Policy").expect("time went backwards"),
            "strict-origin-when-cross-origin"
        );
        assert_eq!(
            headers
                .get("Content-Security-Policy")
                .expect("time went backwards"),
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
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));
        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("time went backwards");
        check_security_headers(response.headers());

        let mut request = Request::builder()
            .uri("/api/models")
            .header("authorization", "Bearer test-token")
            .body(Body::empty())
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));
        let response = app.oneshot(request).await.expect("time went backwards");
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
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));
        let response = app.oneshot(request).await.expect("time went backwards");
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
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));
        let response = app.oneshot(request).await.expect("time went backwards");

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
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));
        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);

        // No auth header — if auth ran before rate-limit, this would
        // return 401 UNAUTHORIZED, not 429 TOO_MANY_REQUESTS.
        let mut request = Request::builder()
            .uri("/api/models")
            .body(Body::empty())
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));
        let response = app.oneshot(request).await.expect("time went backwards");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

        let value = common::read_json::<serde_json::Value>(response).await;
        assert_eq!(value["error"]["type"], common::ERR_RATE_LIMIT);
    }

    #[tokio::test]
    async fn test_rate_limit_applies_to_public_endpoints() {
        let state = create_test_state_overrides(TestOverrides {
            rate_limit: Some(1),
            ..Default::default()
        });
        let app = build_full_app(state);
        let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

        // Exhaust the rate limit on /health
        let mut request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));
        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);

        // Both /health and / should now be rate-limited
        let mut request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));
        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("time went backwards");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

        let mut request = Request::builder()
            .uri("/")
            .body(Body::empty())
            .expect("time went backwards");
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));
        let response = app.oneshot(request).await.expect("time went backwards");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}

// ===================================================================
// Provider adapter integration tests
// ===================================================================

mod provider_integration_tests {
    use super::*;

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
        let body_bytes = serde_json::to_vec(&body).expect("time went backwards");
        let mut req = Request::builder()
            .method("POST")
            .uri("/v1/chat/completions")
            .header("Authorization", format!("Bearer {auth_token}"))
            .header("Content-Type", "application/json")
            .body(Body::from(body_bytes))
            .expect("time went backwards");
        let test_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));
        req.extensions_mut()
            .insert(axum::extract::ConnectInfo(test_addr));
        req
    }

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

        let response = app.oneshot(req).await.expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);

        let json_body = common::read_json::<serde_json::Value>(response).await;
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

        let response = app.oneshot(req).await.expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);

        let json_body = common::read_json::<serde_json::Value>(response).await;
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

        let response = app.oneshot(req).await.expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);

        let json_body = common::read_json::<serde_json::Value>(response).await;
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

        let response = app.oneshot(req).await.expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);

        let json_body = common::read_json::<serde_json::Value>(response).await;
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

        let response = app.oneshot(req).await.expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);

        let json_body = common::read_json::<serde_json::Value>(response).await;
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

        let response = app.oneshot(req).await.expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);

        let json_body = common::read_json::<serde_json::Value>(response).await;
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

        let response = app.oneshot(req).await.expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);

        let json_body = common::read_json::<serde_json::Value>(response).await;
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

        let response = app.oneshot(req).await.expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);

        let json_body = common::read_json::<serde_json::Value>(response).await;

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

        let response = app.oneshot(req).await.expect("time went backwards");
        assert_eq!(response.status(), StatusCode::OK);

        let json_body = common::read_json::<serde_json::Value>(response).await;

        assert_eq!(
            json_body["_gateway"]["classification"]["capabilities"]["vision"], true,
            "Expected vision capability to be true"
        );
        assert_eq!(json_body["model"], "gpt-4-vision");
    }
}
