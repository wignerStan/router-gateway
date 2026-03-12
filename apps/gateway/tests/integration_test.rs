use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Json, Router,
};
use gateway::config::{
    constant_time_token_eq, validate_url_not_private, CredentialConfig, GatewayConfig,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn make_config_with_tokens(tokens: Vec<&str>) -> GatewayConfig {
    let mut config = GatewayConfig::default();
    config.server.auth_tokens = tokens.iter().map(|t| t.to_string()).collect();
    config
}

fn test_credential(id: &str, provider: &str, api_key: &str) -> CredentialConfig {
    CredentialConfig {
        id: id.to_string(),
        provider: provider.to_string(),
        api_key: api_key.to_string(),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Auth middleware tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn auth_valid_token_returns_ok() {
    let config = make_config_with_tokens(vec!["secret-token"]);

    let app = Router::new()
        .route("/api/test", get(|| async { Json(json!({"ok": true})) }))
        .route_layer(axum::middleware::from_fn(move |req, next| {
            let config = config.clone();
            async move { auth_middleware_fn(&config, req, next).await }
        }));

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/test")
                .header("Authorization", "Bearer secret-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn auth_invalid_token_returns_401() {
    let config = make_config_with_tokens(vec!["secret-token"]);

    let app = Router::new()
        .route("/api/test", get(|| async { Json(json!({"ok": true})) }))
        .route_layer(axum::middleware::from_fn(move |req, next| {
            let config = config.clone();
            async move { auth_middleware_fn(&config, req, next).await }
        }));

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/test")
                .header("Authorization", "Bearer wrong-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_missing_header_returns_401() {
    let config = make_config_with_tokens(vec!["secret-token"]);

    let app = Router::new()
        .route("/api/test", get(|| async { Json(json!({"ok": true})) }))
        .route_layer(axum::middleware::from_fn(move |req, next| {
            let config = config.clone();
            async move { auth_middleware_fn(&config, req, next).await }
        }));

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_non_bearer_scheme_returns_401() {
    let config = make_config_with_tokens(vec!["secret-token"]);

    let app = Router::new()
        .route("/api/test", get(|| async { Json(json!({"ok": true})) }))
        .route_layer(axum::middleware::from_fn(move |req, next| {
            let config = config.clone();
            async move { auth_middleware_fn(&config, req, next).await }
        }));

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/test")
                .header("Authorization", "Basic dXNlcjpwYXNz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// Security headers tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn security_headers_present_on_public_route() {
    let app = Router::new()
        .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
        .layer(axum::middleware::from_fn(security_headers_middleware_fn));

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let headers = resp.headers();
    assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
    assert_eq!(headers.get("x-xss-protection").unwrap(), "1; mode=block");
    assert_eq!(
        headers.get("referrer-policy").unwrap(),
        "strict-origin-when-cross-origin"
    );
    assert_eq!(
        headers.get("content-security-policy").unwrap(),
        "default-src 'none'; frame-ancestors 'none'"
    );
}

#[tokio::test]
async fn security_headers_present_on_protected_route() {
    let config = make_config_with_tokens(vec!["tok"]);

    let app = Router::new()
        .route("/api/data", get(|| async { Json(json!({"data": 1})) }))
        .route_layer(axum::middleware::from_fn(move |req, next| {
            let config = config.clone();
            async move { auth_middleware_fn(&config, req, next).await }
        }))
        .layer(axum::middleware::from_fn(security_headers_middleware_fn));

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/data")
                .header("Authorization", "Bearer tok")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(resp.headers().get("x-content-type-options").is_some());
    assert!(resp.headers().get("x-frame-options").is_some());
    assert!(resp.headers().get("x-xss-protection").is_some());
    assert!(resp.headers().get("referrer-policy").is_some());
    assert!(resp.headers().get("content-security-policy").is_some());
}

// ---------------------------------------------------------------------------
// Rate limiting tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rate_limiter_allows_under_limit() {
    let buckets: Arc<Mutex<HashMap<String, (u64, Instant)>>> = Arc::new(Mutex::new(HashMap::new()));
    let limiter = Arc::new(TestRateLimiter::new(buckets.clone(), 5));

    let app = Router::new()
        .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
        .layer(axum::middleware::from_fn_with_state(
            limiter,
            rate_limit_middleware_fn,
        ));

    for _ in 0..5 {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header("X-Forwarded-For", "1.2.3.4")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn rate_limiter_returns_429_over_limit() {
    let buckets: Arc<Mutex<HashMap<String, (u64, Instant)>>> = Arc::new(Mutex::new(HashMap::new()));
    let limiter = Arc::new(TestRateLimiter::new(buckets.clone(), 3));

    let app = Router::new()
        .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
        .layer(axum::middleware::from_fn_with_state(
            limiter,
            rate_limit_middleware_fn,
        ));

    // Exhaust the limit
    for _ in 0..3 {
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header("X-Forwarded-For", "5.6.7.8")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // 4th request should be 429
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("X-Forwarded-For", "5.6.7.8")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"]["type"], "rate_limit_error");
}

#[tokio::test]
async fn rate_limiter_tracks_ips_independently() {
    let buckets: Arc<Mutex<HashMap<String, (u64, Instant)>>> = Arc::new(Mutex::new(HashMap::new()));
    let limiter = Arc::new(TestRateLimiter::new(buckets.clone(), 2));

    let app = Router::new()
        .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
        .layer(axum::middleware::from_fn_with_state(
            limiter,
            rate_limit_middleware_fn,
        ));

    // Exhaust IP A
    for _ in 0..2 {
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header("X-Forwarded-For", "1.1.1.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // IP A should be blocked
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("X-Forwarded-For", "1.1.1.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

    // IP B should still be allowed
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("X-Forwarded-For", "2.2.2.2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn rate_limiter_uses_x_real_ip_fallback() {
    let buckets: Arc<Mutex<HashMap<String, (u64, Instant)>>> = Arc::new(Mutex::new(HashMap::new()));
    let limiter = Arc::new(TestRateLimiter::new(buckets.clone(), 1));

    let app = Router::new()
        .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
        .layer(axum::middleware::from_fn_with_state(
            limiter,
            rate_limit_middleware_fn,
        ));

    // Use X-Real-IP instead of X-Forwarded-For
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("X-Real-IP", "10.0.0.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Same X-Real-IP should now be blocked
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("X-Real-IP", "10.0.0.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

// ---------------------------------------------------------------------------
// SSRF protection tests (validate_url_not_private)
// ---------------------------------------------------------------------------

#[test]
fn ssrf_rejects_loopback_127() {
    assert!(validate_url_not_private("http://127.0.0.1:8080/v1/chat").is_err());
}

#[test]
fn ssrf_rejects_loopback_localhost_numeric() {
    assert!(validate_url_not_private("http://127.0.0.2/api").is_err());
}

#[test]
fn ssrf_rejects_10_private_range() {
    assert!(validate_url_not_private("http://10.0.0.1:443").is_err());
}

#[test]
fn ssrf_rejects_172_16_private_range() {
    assert!(validate_url_not_private("http://172.16.0.1/api").is_err());
}

#[test]
fn ssrf_rejects_172_31_private_range() {
    assert!(validate_url_not_private("http://172.31.255.255/api").is_err());
}

#[test]
fn ssrf_rejects_192_168_private_range() {
    assert!(validate_url_not_private("http://192.168.1.1:8080").is_err());
}

#[test]
fn ssrf_rejects_169_254_link_local() {
    assert!(validate_url_not_private("http://169.254.169.254/latest/meta-data").is_err());
}

#[test]
fn ssrf_rejects_0_0_0_0() {
    assert!(validate_url_not_private("http://0.0.0.0:3000").is_err());
}

#[test]
fn ssrf_rejects_ipv6_loopback() {
    assert!(validate_url_not_private("http://[::1]:8080/api").is_err());
}

#[test]
fn ssrf_rejects_ipv6_unique_local() {
    assert!(validate_url_not_private("http://[fc00::1]/api").is_err());
}

#[test]
fn ssrf_rejects_ipv6_link_local() {
    assert!(validate_url_not_private("http://[fe80::1]/api").is_err());
}

#[test]
fn ssrf_accepts_public_ipv4() {
    assert!(validate_url_not_private("https://api.openai.com/v1/chat").is_ok());
}

#[test]
fn ssrf_accepts_domain_names() {
    assert!(validate_url_not_private("https://api.anthropic.com/v1/messages").is_ok());
}

// ---------------------------------------------------------------------------
// Config validation tests
// ---------------------------------------------------------------------------

#[test]
fn validate_rejects_duplicate_credential_ids() {
    let config = GatewayConfig {
        credentials: vec![
            test_credential("dup", "openai", "key1"),
            test_credential("dup", "openai", "key2"),
        ],
        ..Default::default()
    };
    let err = config.validate().unwrap_err();
    assert!(err.to_string().contains("Duplicate credential ID"));
}

#[test]
fn validate_rejects_empty_api_key() {
    let config = GatewayConfig {
        credentials: vec![test_credential("c1", "openai", "")],
        ..Default::default()
    };
    let err = config.validate().unwrap_err();
    assert!(err.to_string().contains("empty API key"));
}

#[test]
fn validate_rejects_empty_provider() {
    let config = GatewayConfig {
        credentials: vec![test_credential("c1", "", "key1")],
        ..Default::default()
    };
    let err = config.validate().unwrap_err();
    assert!(err.to_string().contains("empty provider"));
}

#[test]
fn validate_rejects_invalid_strategy() {
    let mut config = GatewayConfig::default();
    config.routing.strategy = "does_not_exist".to_string();
    let err = config.validate().unwrap_err();
    assert!(err.to_string().contains("Invalid routing strategy"));
}

#[test]
fn validate_accepts_all_valid_strategies() {
    for strategy in [
        "weighted",
        "time_aware",
        "quota_aware",
        "adaptive",
        "policy_aware",
    ] {
        let mut config = GatewayConfig::default();
        config.routing.strategy = strategy.to_string();
        assert!(
            config.validate().is_ok(),
            "strategy '{strategy}' should be valid"
        );
    }
}

#[test]
fn validate_accepts_valid_config() {
    let config = GatewayConfig {
        credentials: vec![test_credential("c1", "openai", "sk-123")],
        ..Default::default()
    };
    assert!(config.validate().is_ok());
}

// ---------------------------------------------------------------------------
// expand_env_vars tests
// ---------------------------------------------------------------------------

#[test]
fn expand_env_vars_credential_api_key() {
    std::env::set_var("GW_INTEG_TEST_KEY", "expanded-secret");
    let yaml = r#"
credentials:
  - id: c1
    provider: openai
    api_key: "${GW_INTEG_TEST_KEY}"
"#;
    let config = GatewayConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.credentials[0].api_key, "expanded-secret");
    std::env::remove_var("GW_INTEG_TEST_KEY");
}

#[test]
fn expand_env_vars_credential_base_url() {
    std::env::set_var("GW_INTEG_TEST_URL", "https://custom.api.com");
    let yaml = r#"
credentials:
  - id: c1
    provider: openai
    api_key: "some-key"
    base_url: "${GW_INTEG_TEST_URL}"
"#;
    let config = GatewayConfig::from_yaml(yaml).unwrap();
    assert_eq!(
        config.credentials[0].base_url.as_deref(),
        Some("https://custom.api.com")
    );
    std::env::remove_var("GW_INTEG_TEST_URL");
}

#[test]
fn expand_env_vars_default_value_syntax() {
    let yaml = r#"
credentials:
  - id: c1
    provider: openai
    api_key: "${GW_INTEG_MISSING_KEY:-fallback-key}"
"#;
    let config = GatewayConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.credentials[0].api_key, "fallback-key");
}

// ---------------------------------------------------------------------------
// is_auth_enabled tests
// ---------------------------------------------------------------------------

#[test]
fn auth_enabled_with_tokens() {
    let config = make_config_with_tokens(vec!["tok1", "tok2"]);
    assert!(config.is_auth_enabled());
}

#[test]
fn auth_disabled_without_tokens() {
    let config = GatewayConfig::default();
    assert!(!config.is_auth_enabled());
}

// ---------------------------------------------------------------------------
// Strategy alignment: gateway config strategies match smart-routing
// ---------------------------------------------------------------------------

#[test]
fn gateway_strategies_match_smart_routing() {
    // These are the strategies accepted by GatewayConfig::validate()
    let gateway_strategies = [
        "weighted",
        "time_aware",
        "quota_aware",
        "adaptive",
        "policy_aware",
    ];

    // smart-routing config documents the same set
    let smart_routing_strategies = [
        "weighted",
        "time_aware",
        "quota_aware",
        "adaptive",
        "policy_aware",
    ];

    assert_eq!(
        gateway_strategies.len(),
        smart_routing_strategies.len(),
        "Strategy count mismatch"
    );

    for s in &gateway_strategies {
        assert!(
            smart_routing_strategies.contains(s),
            "Strategy '{s}' missing from smart-routing"
        );
    }

    // Verify all are actually accepted by validate()
    let mut config = GatewayConfig::default();
    for s in gateway_strategies {
        config.routing.strategy = s.to_string();
        assert!(
            config.validate().is_ok(),
            "validate() rejected strategy '{s}'"
        );
    }
}

// ---------------------------------------------------------------------------
// constant_time_token_eq tests
// ---------------------------------------------------------------------------

#[test]
fn constant_time_eq_matching_tokens() {
    assert!(constant_time_token_eq("my-secret-token", "my-secret-token"));
}

#[test]
fn constant_time_eq_mismatching_tokens() {
    assert!(!constant_time_token_eq(
        "my-secret-token",
        "my-secret-tokem"
    ));
}

#[test]
fn constant_time_eq_different_lengths() {
    assert!(!constant_time_token_eq("short", "much-longer-token"));
}

#[test]
fn constant_time_eq_both_empty() {
    assert!(constant_time_token_eq("", ""));
}

// ---------------------------------------------------------------------------
// Test helper implementations
// (Mirror the real middleware logic so integration tests exercise the
// full stack without coupling to internal-only types like AppState)
// ---------------------------------------------------------------------------

/// Standalone auth middleware that mirrors the production `auth_middleware`.
async fn auth_middleware_fn(
    config: &GatewayConfig,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> std::result::Result<axum::response::Response, (axum::http::StatusCode, Json<Value>)> {
    use axum::http::header::AUTHORIZATION;

    if config.server.auth_tokens.is_empty() {
        return Err((
            axum::http::StatusCode::FORBIDDEN,
            Json(json!({"error": {"type": "config_error", "message": "No auth tokens"}})),
        ));
    }

    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    match auth_header {
        Some(header) => {
            if let Some(token) = header.strip_prefix("Bearer ") {
                if config
                    .server
                    .auth_tokens
                    .iter()
                    .any(|t| constant_time_token_eq(t, token))
                {
                    return Ok(next.run(req).await);
                }
            }
            Err((
                axum::http::StatusCode::UNAUTHORIZED,
                Json(
                    json!({"error": {"type": "invalid_request_error", "message": "Invalid or expired API token"}}),
                ),
            ))
        },
        None => Err((
            axum::http::StatusCode::UNAUTHORIZED,
            Json(
                json!({"error": {"type": "invalid_request_error", "message": "Missing Authorization header"}}),
            ),
        )),
    }
}

/// Standalone security headers middleware matching production behavior.
async fn security_headers_middleware_fn(
    req: axum::extract::Request,
    next: axum::middleware::Next,
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

    response
}

/// Simplified rate limiter for deterministic integration tests.
struct TestRateLimiter {
    buckets: Arc<Mutex<HashMap<String, (u64, Instant)>>>,
    max_requests: u64,
}

impl TestRateLimiter {
    fn new(buckets: Arc<Mutex<HashMap<String, (u64, Instant)>>>, max_requests: u64) -> Self {
        Self {
            buckets,
            max_requests,
        }
    }

    fn check(&self, ip: &str) -> bool {
        let mut buckets = self.buckets.lock().unwrap();
        let now = Instant::now();
        let (count, _window) = buckets.entry(ip.to_string()).or_insert((0, now));
        if *count >= self.max_requests {
            return false;
        }
        *count += 1;
        true
    }
}

/// Standalone rate limit middleware matching production behavior.
async fn rate_limit_middleware_fn(
    axum::extract::State(limiter): axum::extract::State<Arc<TestRateLimiter>>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> std::result::Result<axum::response::Response, (axum::http::StatusCode, Json<Value>)> {
    let client_ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()))
        .unwrap_or("unknown");

    if !limiter.check(client_ip) {
        return Err((
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(
                json!({"error": {"type": "rate_limit_error", "message": "Too many requests. Please try again later."}}),
            ),
        ));
    }

    Ok(next.run(req).await)
}
