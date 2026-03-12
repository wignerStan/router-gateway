mod common;

use serde_json::json;

use common::{spawn_gateway, test_config_with_auth, test_credentials};

#[tokio::test]
async fn chat_completions_returns_200_with_valid_model() {
    let mut config = test_config_with_auth("token");
    config.credentials = test_credentials();
    let server = spawn_gateway(config).await;

    let resp = server
        .client
        .post(format!("{}/v1/chat/completions", server.url))
        .header("Authorization", "Bearer token")
        .json(&json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["object"], "chat.completion");
    assert_eq!(body["model"], "gpt-4");
    assert_eq!(body["choices"][0]["message"]["role"], "assistant");
    assert!(body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap()
        .contains("Gateway mock response"));

    server.shutdown();
}

#[tokio::test]
async fn chat_completions_routes_to_correct_provider() {
    let mut config = test_config_with_auth("token");
    config.credentials = test_credentials();
    let server = spawn_gateway(config).await;

    let resp = server
        .client
        .post(format!("{}/v1/chat/completions", server.url))
        .header("Authorization", "Bearer token")
        .json(&json!({
            "model": "gpt-3.5-turbo",
            "messages": [{"role": "user", "content": "Hi"}]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["_gateway"]["route"]["provider"], "openai");
    assert_eq!(body["_gateway"]["route"]["credential_id"], "openai-primary");

    server.shutdown();
}

#[tokio::test]
async fn chat_completions_no_credentials_returns_503() {
    let config = test_config_with_auth("token");
    let server = spawn_gateway(config).await;

    let resp = server
        .client
        .post(format!("{}/v1/chat/completions", server.url))
        .header("Authorization", "Bearer token")
        .json(&json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 503);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "no_route_available");

    server.shutdown();
}

#[tokio::test]
async fn chat_completions_unknown_model_falls_back_to_first_credential() {
    let mut config = test_config_with_auth("token");
    config.credentials = test_credentials();
    let server = spawn_gateway(config).await;

    let resp = server
        .client
        .post(format!("{}/v1/chat/completions", server.url))
        .header("Authorization", "Bearer token")
        .json(&json!({
            "model": "nonexistent-model",
            "messages": [{"role": "user", "content": "Hello"}]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["model"], "nonexistent-model");
    assert_eq!(body["_gateway"]["route"]["credential_id"], "openai-primary");

    server.shutdown();
}

#[tokio::test]
async fn list_models_returns_configured_models() {
    let mut config = test_config_with_auth("token");
    config.credentials = test_credentials();
    let server = spawn_gateway(config).await;

    let resp = server
        .client
        .get(format!("{}/api/models", server.url))
        .header("Authorization", "Bearer token")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["count"].as_u64().unwrap() > 0);
    assert_eq!(body["message"], "Models loaded from configuration");

    server.shutdown();
}

#[tokio::test]
async fn list_models_empty_when_no_credentials() {
    let config = test_config_with_auth("token");
    let server = spawn_gateway(config).await;

    let resp = server
        .client
        .get(format!("{}/api/models", server.url))
        .header("Authorization", "Bearer token")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["count"], 0);
    assert_eq!(body["message"], "No models configured");

    server.shutdown();
}

#[tokio::test]
async fn route_endpoint_returns_plan_with_credentials() {
    let mut config = test_config_with_auth("token");
    config.credentials = test_credentials();
    let server = spawn_gateway(config).await;

    let resp = server
        .client
        .get(format!("{}/api/route", server.url))
        .header("Authorization", "Bearer token")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["message"], "Route planned successfully");
    assert!(body["route_plan"]["primary"].is_object());
    assert_eq!(body["route_plan"]["total_candidates"], 3);

    server.shutdown();
}

#[tokio::test]
async fn route_endpoint_no_routes_when_no_credentials() {
    let config = test_config_with_auth("token");
    let server = spawn_gateway(config).await;

    let resp = server
        .client
        .get(format!("{}/api/route", server.url))
        .header("Authorization", "Bearer token")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["message"], "No suitable routes found");
    assert!(body["route_plan"]["primary"].is_null());

    server.shutdown();
}
