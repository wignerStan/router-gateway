mod common;

use common::{spawn_gateway, test_config_with_auth, test_credentials};

#[tokio::test]
async fn health_returns_200() {
    let server = spawn_gateway(test_config_with_auth("t")).await;

    let resp = server
        .client
        .get(format!("{}/health", server.url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    server.shutdown();
}

#[tokio::test]
async fn health_contains_expected_fields() {
    let mut config = test_config_with_auth("t");
    config.credentials = test_credentials();
    let server = spawn_gateway(config).await;

    let resp = server
        .client
        .get(format!("{}/health", server.url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();

    assert_eq!(body["status"], "healthy");
    assert_eq!(body["credential_count"], 3);
    assert_eq!(body["healthy_count"], 3);
    assert_eq!(body["degraded_count"], 0);
    assert_eq!(body["unhealthy_count"], 0);
    // uptime should be small for a fresh server
    assert!(body["uptime_secs"].as_u64().unwrap() < 5);

    server.shutdown();
}

#[tokio::test]
async fn root_returns_gateway_info() {
    let server = spawn_gateway(test_config_with_auth("t")).await;

    let resp = server
        .client
        .get(format!("{}/", server.url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["name"], "Gateway API");
    assert_eq!(body["version"], "0.1.0");

    server.shutdown();
}

#[tokio::test]
async fn health_has_security_headers() {
    let server = spawn_gateway(test_config_with_auth("t")).await;

    let resp = server
        .client
        .get(format!("{}/health", server.url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let headers = resp.headers();
    assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
    assert_eq!(headers.get("x-xss-protection").unwrap(), "1; mode=block");
    assert_eq!(
        headers.get("referrer-policy").unwrap(),
        "strict-origin-when-cross-origin"
    );

    server.shutdown();
}

#[tokio::test]
async fn health_reflects_credential_count() {
    // No credentials
    let server = spawn_gateway(test_config_with_auth("t")).await;

    let resp = server
        .client
        .get(format!("{}/health", server.url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["credential_count"], 0);

    server.shutdown();

    // With credentials
    let mut config = test_config_with_auth("t");
    config.credentials = test_credentials();
    let server = spawn_gateway(config).await;

    let resp = server
        .client
        .get(format!("{}/health", server.url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["credential_count"], 3);

    server.shutdown();
}
