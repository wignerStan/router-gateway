mod common;

use common::{
    spawn_gateway, spawn_gateway_with_rate_limit, test_config_no_auth, test_config_with_auth,
};

#[tokio::test]
async fn missing_auth_header_returns_401() {
    let config = test_config_with_auth("secret");
    let server = spawn_gateway(config).await;

    let resp = server
        .client
        .get(format!("{}/api/models", server.url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Missing Authorization header"));

    server.shutdown();
}

#[tokio::test]
async fn wrong_token_returns_401() {
    let config = test_config_with_auth("correct-token");
    let server = spawn_gateway(config).await;

    let resp = server
        .client
        .get(format!("{}/api/models", server.url))
        .header("Authorization", "Bearer wrong-token")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "invalid_request_error");

    server.shutdown();
}

#[tokio::test]
async fn valid_token_returns_200() {
    let config = test_config_with_auth("my-token");
    let server = spawn_gateway(config).await;

    let resp = server
        .client
        .get(format!("{}/api/models", server.url))
        .header("Authorization", "Bearer my-token")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    server.shutdown();
}

#[tokio::test]
async fn no_tokens_configured_returns_403() {
    let config = test_config_no_auth();
    let server = spawn_gateway(config).await;

    let resp = server
        .client
        .get(format!("{}/api/models", server.url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 403);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "config_error");

    server.shutdown();
}

#[tokio::test]
async fn public_routes_bypass_auth() {
    let config = test_config_with_auth("secret");
    let server = spawn_gateway(config).await;

    // /health is public — no auth needed
    let resp = server
        .client
        .get(format!("{}/health", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // / is also public
    let resp = server
        .client
        .get(format!("{}/", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    server.shutdown();
}

#[tokio::test]
async fn rate_limit_returns_429_after_threshold() {
    let config = test_config_no_auth();
    let server = spawn_gateway_with_rate_limit(config, 3).await;

    // First 3 requests should succeed
    for _ in 0..3 {
        let resp = server
            .client
            .get(format!("{}/health", server.url))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    // 4th request should be rate limited
    let resp = server
        .client
        .get(format!("{}/health", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 429);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "rate_limit_error");

    server.shutdown();
}

#[tokio::test]
async fn different_ips_have_independent_rate_limits() {
    let config = test_config_no_auth();
    let server = spawn_gateway_with_rate_limit(config, 2).await;

    // Exhaust limit for IP A
    for _ in 0..2 {
        let resp = server
            .client
            .get(format!("{}/health", server.url))
            .header("X-Forwarded-For", "1.1.1.1")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    // IP A is blocked
    let resp = server
        .client
        .get(format!("{}/health", server.url))
        .header("X-Forwarded-For", "1.1.1.1")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 429);

    // IP B is still allowed
    let resp = server
        .client
        .get(format!("{}/health", server.url))
        .header("X-Forwarded-For", "2.2.2.2")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    server.shutdown();
}
