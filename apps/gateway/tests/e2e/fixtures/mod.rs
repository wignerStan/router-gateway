use gateway::config::{CredentialConfig, GatewayConfig};

/// Shared test auth token used across E2E tests.
pub const MOCK_AUTH_TOKEN: &str = "test-e2e-token";

/// Build a [`GatewayConfig`] with auth enabled and sample credentials.
pub fn config_with_credentials() -> GatewayConfig {
    let mut config = GatewayConfig::default();
    config.server.auth_tokens = vec![MOCK_AUTH_TOKEN.to_string()];
    config.credentials = multi_provider_credentials();
    config
}

/// Build a [`GatewayConfig`] with auth enabled but no credentials.
pub fn config_no_credentials() -> GatewayConfig {
    let mut config = GatewayConfig::default();
    config.server.auth_tokens = vec![MOCK_AUTH_TOKEN.to_string()];
    config
}

/// Sample credentials spanning multiple providers for testing routing.
pub fn multi_provider_credentials() -> Vec<CredentialConfig> {
    vec![
        CredentialConfig {
            id: "openai-primary".into(),
            provider: "openai".into(),
            api_key: "sk-test-primary".into(),
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
            allowed_models: vec!["claude-3-opus".into(), "claude-3-sonnet".into()],
            priority: 8,
            ..Default::default()
        },
        CredentialConfig {
            id: "google-primary".into(),
            provider: "google".into(),
            api_key: "test-google-key".into(),
            allowed_models: vec!["gemini-pro".into()],
            priority: 7,
            ..Default::default()
        },
    ]
}

/// A minimal chat completions request body for testing.
pub fn sample_chat_request() -> serde_json::Value {
    serde_json::json!({
        "model": "gpt-4",
        "messages": [
            { "role": "user", "content": "Hello" }
        ],
        "max_tokens": 100
    })
}

/// A chat completions request with an unknown model.
pub fn sample_unknown_model_request() -> serde_json::Value {
    serde_json::json!({
        "model": "nonexistent-model",
        "messages": [
            { "role": "user", "content": "Hello" }
        ]
    })
}
