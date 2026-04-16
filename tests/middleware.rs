#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Isolated middleware tests.
//!
//! Each middleware is tested in isolation by mounting it on a minimal Axum
//! router with a single `ok_handler` that returns 200 "ok". This avoids
//! coupling middleware behaviour to specific route handlers.

use axum::{
    Router,
    body::Body,
    extract::Request,
    http::{StatusCode, header},
    middleware,
    routing::get,
};
use gateway::build_app_state;
use gateway::config::GatewayConfig;
use gateway::routes::{auth_middleware, rate_limit_middleware, security_headers_middleware};
use gateway::state::AppState;
use serde_json::Value;
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const MAX_RESPONSE_BYTES: usize = 4096;

/// Minimal handler that always returns 200 "ok".
async fn ok_handler() -> &'static str {
    "ok"
}

/// Read and deserialize the JSON body of a response.
async fn read_json_body<T: serde::de::DeserializeOwned>(response: axum::response::Response) -> T {
    let body_bytes = axum::body::to_bytes(response.into_body(), MAX_RESPONSE_BYTES)
        .await
        .expect("response body should be readable");
    serde_json::from_slice(&body_bytes).unwrap_or_else(|e| {
        panic!(
            "Failed to deserialize JSON: {e}. Body: {}",
            String::from_utf8_lossy(&body_bytes)
        )
    })
}

/// Overrides for constructing test `AppState`.
struct TestOverrides {
    auth_tokens: Vec<String>,
    rate_limit: Option<u64>,
    trust_proxy_headers: bool,
}

impl Default for TestOverrides {
    fn default() -> Self {
        Self {
            auth_tokens: vec!["test-token".to_string()],
            rate_limit: None,
            trust_proxy_headers: false,
        }
    }
}

/// Build an `AppState` with the given overrides, using `build_app_state` so
/// production and test construction stay in sync.
fn create_test_state(overrides: TestOverrides) -> AppState {
    let mut config = GatewayConfig::default();
    config.server.auth_tokens = overrides.auth_tokens;
    config.server.trust_proxy_headers = overrides.trust_proxy_headers;
    build_app_state(config, overrides.rate_limit)
}

/// Standard test address used for `ConnectInfo`.
fn test_addr() -> std::net::SocketAddr {
    std::net::SocketAddr::from(([127, 0, 0, 1], 12345))
}

/// Build a request with `ConnectInfo` set.
fn make_request(uri: &str) -> Request<Body> {
    let mut req = Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("building request should not fail");
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(test_addr()));
    req
}

/// Build a request with `ConnectInfo` and an Authorization header.
fn make_request_with_auth(uri: &str, token: &str) -> Request<Body> {
    let mut req = Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("building request should not fail");
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(test_addr()));
    req
}

/// Build a request with `ConnectInfo` and a specific source address.
fn make_request_from_addr(uri: &str, addr: std::net::SocketAddr) -> Request<Body> {
    let mut req = Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("building request should not fail");
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(addr));
    req
}

/// Build a request with `ConnectInfo`, custom headers, and a specific source address.
fn make_request_with_headers_and_addr(
    uri: &str,
    headers: &[(&str, &str)],
    addr: std::net::SocketAddr,
) -> Request<Body> {
    let mut builder = Request::builder().uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let mut req = builder
        .body(Body::empty())
        .expect("building request should not fail");
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(addr));
    req
}

// ---------------------------------------------------------------------------
// Error type constants (mirrors production values)
// ---------------------------------------------------------------------------

const ERR_INVALID_REQUEST: &str = "invalid_request_error";
const ERR_CONFIG_ERROR: &str = "config_error";
const ERR_RATE_LIMIT: &str = "rate_limit_error";

// ===================================================================
// Auth middleware (isolated)
// ===================================================================

mod auth_middleware_tests {
    use super::*;

    /// Build a minimal router with only the auth middleware wrapping `ok_handler`.
    fn auth_only_router(state: AppState) -> Router {
        Router::new()
            .route("/test", get(ok_handler))
            .route_layer(middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state)
    }

    #[tokio::test]
    async fn valid_bearer_token_passes() {
        let state = create_test_state(TestOverrides::default());
        let app = auth_only_router(state);

        let response = app
            .oneshot(make_request_with_auth("/test", "test-token"))
            .await
            .expect("oneshot should succeed");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn missing_auth_returns_401() {
        let state = create_test_state(TestOverrides::default());
        let app = auth_only_router(state);

        let response = app
            .oneshot(make_request("/test"))
            .await
            .expect("oneshot should succeed");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body: Value = read_json_body(response).await;
        assert_eq!(body["error"]["type"], ERR_INVALID_REQUEST);
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("message should be a string")
                .contains("Missing Authorization header")
        );
    }

    #[tokio::test]
    async fn invalid_token_returns_401() {
        let state = create_test_state(TestOverrides::default());
        let app = auth_only_router(state);

        let response = app
            .oneshot(make_request_with_auth("/test", "wrong-token"))
            .await
            .expect("oneshot should succeed");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body: Value = read_json_body(response).await;
        assert_eq!(body["error"]["type"], ERR_INVALID_REQUEST);
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("message should be a string")
                .contains("Invalid or expired API token")
        );
    }

    #[tokio::test]
    async fn no_tokens_configured_returns_403() {
        let state = create_test_state(TestOverrides {
            auth_tokens: vec![],
            ..TestOverrides::default()
        });
        let app = auth_only_router(state);

        let response = app
            .oneshot(make_request_with_auth("/test", "any-token"))
            .await
            .expect("oneshot should succeed");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let body: Value = read_json_body(response).await;
        assert_eq!(body["error"]["type"], ERR_CONFIG_ERROR);
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("message should be a string")
                .contains("improperly configured")
        );
    }

    #[tokio::test]
    async fn multiple_tokens_any_valid() {
        let state = create_test_state(TestOverrides {
            auth_tokens: vec![
                "alpha-token".to_string(),
                "beta-token".to_string(),
                "gamma-token".to_string(),
            ],
            ..TestOverrides::default()
        });
        let app = auth_only_router(state);

        // First token works
        let response = app
            .clone()
            .oneshot(make_request_with_auth("/test", "alpha-token"))
            .await
            .expect("oneshot should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        // Second token works
        let response = app
            .clone()
            .oneshot(make_request_with_auth("/test", "beta-token"))
            .await
            .expect("oneshot should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        // Third token works
        let response = app
            .oneshot(make_request_with_auth("/test", "gamma-token"))
            .await
            .expect("oneshot should succeed");
        assert_eq!(response.status(), StatusCode::OK);
    }
}

// ===================================================================
// Rate limit middleware (isolated)
// ===================================================================

mod rate_limit_middleware_tests {
    use super::*;

    /// Build a minimal router with only the rate-limit middleware wrapping `ok_handler`.
    fn rate_limit_only_router(state: AppState) -> Router {
        Router::new()
            .route("/test", get(ok_handler))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                rate_limit_middleware,
            ))
            .with_state(state)
    }

    #[tokio::test]
    async fn under_limit_passes() {
        let state = create_test_state(TestOverrides {
            rate_limit: Some(5),
            ..TestOverrides::default()
        });
        let app = rate_limit_only_router(state);

        for _ in 0..5 {
            let response = app
                .clone()
                .oneshot(make_request("/test"))
                .await
                .expect("oneshot should succeed");
            assert_eq!(response.status(), StatusCode::OK);
        }
    }

    #[tokio::test]
    async fn over_limit_returns_429() {
        let state = create_test_state(TestOverrides {
            rate_limit: Some(3),
            ..TestOverrides::default()
        });
        let app = rate_limit_only_router(state);

        // Exhaust the limit
        for _ in 0..3 {
            let response = app
                .clone()
                .oneshot(make_request("/test"))
                .await
                .expect("oneshot should succeed");
            assert_eq!(response.status(), StatusCode::OK);
        }

        // Next request should be rejected
        let response = app
            .oneshot(make_request("/test"))
            .await
            .expect("oneshot should succeed");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

        let body: Value = read_json_body(response).await;
        assert_eq!(body["error"]["type"], ERR_RATE_LIMIT);
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("message should be a string")
                .contains("Too many requests")
        );
    }

    #[tokio::test]
    async fn different_ips_independent() {
        let state = create_test_state(TestOverrides {
            rate_limit: Some(1),
            ..TestOverrides::default()
        });
        let app = rate_limit_only_router(state);

        let addr_a: std::net::SocketAddr = "127.0.0.2:12345".parse().unwrap();
        let addr_b: std::net::SocketAddr = "127.0.0.3:12345".parse().unwrap();

        // First request from IP A passes
        let response = app
            .clone()
            .oneshot(make_request_from_addr("/test", addr_a))
            .await
            .expect("oneshot should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        // Second request from IP A is rejected
        let response = app
            .clone()
            .oneshot(make_request_from_addr("/test", addr_a))
            .await
            .expect("oneshot should succeed");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

        // First request from IP B passes (independent bucket)
        let response = app
            .oneshot(make_request_from_addr("/test", addr_b))
            .await
            .expect("oneshot should succeed");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn x_forwarded_for_with_trust_proxy() {
        let state = create_test_state(TestOverrides {
            rate_limit: Some(1),
            trust_proxy_headers: true,
            ..TestOverrides::default()
        });
        let app = rate_limit_only_router(state);

        let real_addr: std::net::SocketAddr = "127.0.0.1:12345".parse().unwrap();
        let forwarded_ip = "10.0.0.99";

        // Request with X-Forwarded-For should be bucketed on the forwarded IP
        let response = app
            .clone()
            .oneshot(make_request_with_headers_and_addr(
                "/test",
                &[("x-forwarded-for", forwarded_ip)],
                real_addr,
            ))
            .await
            .expect("oneshot should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        // Same forwarded IP — should be rejected (same bucket)
        let response = app
            .clone()
            .oneshot(make_request_with_headers_and_addr(
                "/test",
                &[("x-forwarded-for", forwarded_ip)],
                real_addr,
            ))
            .await
            .expect("oneshot should succeed");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

        // Different forwarded IP — should pass (different bucket)
        let response = app
            .oneshot(make_request_with_headers_and_addr(
                "/test",
                &[("x-forwarded-for", "10.0.0.200")],
                real_addr,
            ))
            .await
            .expect("oneshot should succeed");
        assert_eq!(response.status(), StatusCode::OK);
    }
}

// ===================================================================
// Security headers middleware (isolated)
// ===================================================================

mod security_headers_middleware_tests {
    use super::*;

    /// Build a minimal router with only the security-headers middleware.
    fn security_headers_only_router() -> Router {
        Router::new()
            .route("/test", get(ok_handler))
            .layer(middleware::from_fn(security_headers_middleware))
    }

    /// Assert all expected security headers are present and correct.
    fn assert_security_headers(headers: &axum::http::HeaderMap) {
        assert_eq!(
            headers
                .get("x-content-type-options")
                .expect("X-Content-Type-Options should be present"),
            "nosniff"
        );
        assert_eq!(
            headers
                .get("x-frame-options")
                .expect("X-Frame-Options should be present"),
            "DENY"
        );
        assert_eq!(
            headers
                .get("referrer-policy")
                .expect("Referrer-Policy should be present"),
            "strict-origin-when-cross-origin"
        );
        assert_eq!(
            headers
                .get("content-security-policy")
                .expect("Content-Security-Policy should be present"),
            "default-src 'none'; frame-ancestors 'none'"
        );
    }

    #[tokio::test]
    async fn all_security_headers_present() {
        let app = security_headers_only_router();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .body(Body::empty())
                    .expect("building request should not fail"),
            )
            .await
            .expect("oneshot should succeed");

        assert_eq!(response.status(), StatusCode::OK);
        assert_security_headers(response.headers());
    }

    #[tokio::test]
    async fn headers_present_on_error() {
        // Mount the security headers middleware around a handler that returns
        // an error (404 from a non-existent route). The middleware wraps the
        // entire router, so security headers should still be applied even
        // though the inner handler returned an error response.
        let app = Router::new()
            .route("/exists", get(ok_handler))
            .layer(middleware::from_fn(security_headers_middleware));

        // Request a path that does NOT have a route — Axum returns 404.
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/nonexistent")
                    .body(Body::empty())
                    .expect("building request should not fail"),
            )
            .await
            .expect("oneshot should succeed");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_security_headers(response.headers());
    }
}
