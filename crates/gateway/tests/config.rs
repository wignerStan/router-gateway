#![allow(missing_docs, clippy::expect_used)]
use gateway::config::GatewayConfig;
use std::io::Write;
use tempfile::NamedTempFile;

fn config_from_yaml_content(yaml_content: &str) -> GatewayConfig {
    let mut tmp_file = NamedTempFile::new().expect("failed to create temp file");
    write!(tmp_file, "{yaml_content}").expect("failed to write temp file");
    GatewayConfig::from_file(tmp_file.path()).expect("failed to load config from file")
}

#[test]
fn test_default_config() {
    let config = GatewayConfig::default();
    assert_eq!(config.server.port, 3000);
    assert_eq!(config.server.host, "0.0.0.0");
    assert!(config.credentials.is_empty());
}

#[test]
fn test_parse_minimal_yaml() {
    let yaml = r"
server:
  port: 8080
";
    let config = GatewayConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.server.port, 8080);
}

#[test]
fn test_parse_credentials() {
    let yaml = r"
credentials:
  - id: anthropic-primary
    provider: anthropic
    api_key: sk-test-key
    priority: 10
";
    let config = GatewayConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.credentials.len(), 1);
    assert_eq!(config.credentials[0].id, "anthropic-primary");
    assert_eq!(config.credentials[0].provider, "anthropic");
    assert_eq!(config.credentials[0].priority, 10);
}

#[test]
fn test_duplicate_credential_id_fails() {
    let yaml = r"
credentials:
  - id: test-cred
    provider: anthropic
    api_key: key1
  - id: test-cred
    provider: openai
    api_key: key2
";
    let result = GatewayConfig::from_yaml(yaml);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Duplicate credential ID"));
}

#[test]
fn test_invalid_strategy_fails() {
    let yaml = r"
routing:
  strategy: invalid
";
    let result = GatewayConfig::from_yaml(yaml);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Invalid routing strategy"));
}

#[test]
fn test_provider_filtering() {
    let config = GatewayConfig::from_yaml(
        r"
credentials:
  - id: cred1
    provider: anthropic
    api_key: key1
  - id: cred2
    provider: openai
    api_key: key2
  - id: cred3
    provider: anthropic
    api_key: key3
",
    )
    .unwrap();

    let anthropic_creds = config.credentials_for_provider("anthropic");
    assert_eq!(anthropic_creds.len(), 2);

    let openai_creds = config.credentials_for_provider("openai");
    assert_eq!(openai_creds.len(), 1);
}

// --- SSRF protection tests ---

#[test]
fn test_ssrf_reject_loopback() {
    let yaml = r"
credentials:
  - id: test
    provider: openai
    api_key: key1
    base_url: http://127.0.0.1:8000
";
    let result = GatewayConfig::from_yaml(yaml);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("private/internal IP") || err_msg.contains("invalid base_url"),
        "Unexpected error: {err_msg}"
    );
}

#[test]
fn test_provider_env_vars_with_defaults() {
    let config = GatewayConfig::from_yaml(
        r"
credentials:
  - id: cred1
    provider: openai
    api_key: key1
providers:
  openai:
    enabled: true
    base_url: ${PROVIDER_BASE_URL:-https://api.openai.com}
    headers:
      Authorization: Bearer ${PROVIDER_AUTH}
",
    )
    .unwrap();

    let openai = config.providers.get("openai").unwrap();
    assert_eq!(openai.base_url.as_deref(), Some("https://api.openai.com"));
    assert_eq!(
        openai.headers.get("Authorization").unwrap(),
        "Bearer ",
        "Unset env var should expand to empty string"
    );
}

#[test]
fn test_ssrf_allow_credential_without_base_url() {
    let yaml = r"
credentials:
  - id: test
    provider: openai
    api_key: key1
";
    let result = GatewayConfig::from_yaml(yaml);
    assert!(result.is_ok());
}

// --- Auth enabled tests ---

#[test]
fn test_auth_disabled_when_no_tokens() {
    let config = GatewayConfig::default();
    assert!(!config.is_auth_enabled());
}

#[test]
fn test_auth_enabled_with_tokens() {
    let yaml = r#"
server:
  auth_tokens:
    - "my-secret-token"
"#;
    let config = GatewayConfig::from_yaml(yaml).unwrap();
    assert!(config.is_auth_enabled());
}

#[test]
fn test_provider_config_without_env_vars_unchanged() {
    let config = GatewayConfig::from_yaml(
        r"
credentials:
  - id: cred1
    provider: openai
    api_key: key1
providers:
  openai:
    enabled: true
    base_url: https://api.openai.com
    headers:
      X-Custom: literal-value
",
    )
    .unwrap();

    let openai = config.providers.get("openai").unwrap();
    assert_eq!(openai.base_url.as_deref(), Some("https://api.openai.com"));
    assert_eq!(openai.headers.get("X-Custom").unwrap(), "literal-value");
}

// --- Config File Integration Tests ---

#[test]
fn test_from_file_valid_yaml() {
    let yaml_content = r"
server:
  port: 9090
  host: 127.0.0.1
  timeout_secs: 60
credentials:
  - id: openai-prod
    provider: openai
    api_key: sk-test-key-123 # gitleaks:allow
    priority: 5
";
    let config = config_from_yaml_content(yaml_content);

    assert_eq!(config.server.port, 9090);
    assert_eq!(config.server.host, "127.0.0.1");
    assert_eq!(config.server.timeout_secs, 60);
    assert_eq!(config.credentials.len(), 1);
    assert_eq!(config.credentials[0].id, "openai-prod");
    assert_eq!(config.credentials[0].provider, "openai");
    assert_eq!(config.credentials[0].api_key, "sk-test-key-123");
    assert_eq!(config.credentials[0].priority, 5);
}

#[test]
fn test_from_file_nonexistent() {
    let result = GatewayConfig::from_file("nonexistent.yaml");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Failed to read configuration file"));
}

#[test]
fn test_full_config_with_all_sections() {
    let yaml_content = r"
server:
  port: 8080
  host: 0.0.0.0
  timeout_secs: 90
  auth_tokens:
    - token-one
    - token-two
  trust_proxy_headers: true

credentials:
  - id: openai-primary
    provider: openai
    api_key: sk-openai-key # gitleaks:allow
    priority: 10
    allowed_models:
      - gpt-4
      - gpt-3.5-turbo
  - id: google-primary
    provider: google
    api_key: google-key-123 # gitleaks:allow
    priority: 8

routing:
  strategy: adaptive
  session_affinity: false
  min_healthy_credentials: 2
  fallback_depth: 3

providers:
  openai:
    enabled: true
    base_url: https://api.openai.com/v1
    timeout_secs: 120
    headers:
      X-Custom-Header: custom-value
  google:
    enabled: true
    base_url: https://generativelanguage.googleapis.com/v1beta
";
    let config = config_from_yaml_content(yaml_content);

    // Server section
    assert_eq!(config.server.port, 8080);
    assert_eq!(config.server.host, "0.0.0.0");
    assert_eq!(config.server.timeout_secs, 90);
    assert_eq!(config.server.auth_tokens, vec!["token-one", "token-two"]);
    assert!(config.server.trust_proxy_headers);

    // Credentials section
    assert_eq!(config.credentials.len(), 2);
    assert_eq!(config.credentials[0].id, "openai-primary");
    assert_eq!(
        config.credentials[0].allowed_models,
        vec!["gpt-4", "gpt-3.5-turbo"]
    );
    assert_eq!(config.credentials[1].id, "google-primary");
    assert_eq!(config.credentials[1].provider, "google");

    // Routing section
    assert_eq!(config.routing.strategy, "adaptive");
    assert!(!config.routing.session_affinity);
    assert_eq!(config.routing.min_healthy_credentials, 2);
    assert_eq!(config.routing.fallback_depth, 3);

    // Providers section
    let openai_provider = config.providers.get("openai").unwrap();
    assert!(openai_provider.enabled);
    assert_eq!(openai_provider.timeout_secs, Some(120));
    assert_eq!(
        openai_provider.headers.get("X-Custom-Header").unwrap(),
        "custom-value"
    );

    let google_provider = config.providers.get("google").unwrap();
    assert!(google_provider.enabled);
    assert_eq!(
        google_provider.base_url.as_deref(),
        Some("https://generativelanguage.googleapis.com/v1beta")
    );
}

#[test]
fn test_default_routing_policy() {
    let config = GatewayConfig::default();
    assert_eq!(config.routing.strategy, "weighted");
    assert!(config.routing.session_affinity);
}

#[test]
fn test_credential_with_all_fields() {
    let yaml_content = r"
credentials:
  - id: full-cred
    provider: openai
    api_key: sk-comprehensive-key # gitleaks:allow
    base_url: https://custom.openai.proxy.com
    organization: org-example-123
    allowed_models:
      - gpt-4
      - gpt-4-turbo
    priority: 15
    daily_quota: 10000
    rate_limit: 60
";
    let config = config_from_yaml_content(yaml_content);

    let cred = &config.credentials[0];
    assert_eq!(cred.id, "full-cred");
    assert_eq!(cred.provider, "openai");
    assert_eq!(cred.api_key, "sk-comprehensive-key");
    assert_eq!(
        cred.base_url.as_deref(),
        Some("https://custom.openai.proxy.com")
    );
    assert_eq!(cred.organization.as_deref(), Some("org-example-123"));
    assert_eq!(cred.allowed_models, vec!["gpt-4", "gpt-4-turbo"]);
    assert_eq!(cred.priority, 15);
    assert_eq!(cred.daily_quota, Some(10000));
    assert_eq!(cred.rate_limit, Some(60));
}

#[test]
fn test_multiple_providers_config() {
    let yaml_content = r#"
credentials:
  - id: c1
    provider: openai
    api_key: key1
  - id: c2
    provider: google
    api_key: key2
  - id: c3
    provider: openai
    api_key: key3
providers:
  openai:
    enabled: true
    base_url: https://api.openai.com/v1
  google:
    enabled: false
  custom-llm:
    enabled: true
    base_url: https://custom-llm.example.com
    headers:
      X-API-Version: "2024-01"
"#;
    let config = config_from_yaml_content(yaml_content);

    assert_eq!(config.providers.len(), 3);

    let openai = config.providers.get("openai").unwrap();
    assert!(openai.enabled);
    assert_eq!(
        openai.base_url.as_deref(),
        Some("https://api.openai.com/v1")
    );

    let google = config.providers.get("google").unwrap();
    assert!(!google.enabled);

    let custom = config.providers.get("custom-llm").unwrap();
    assert!(custom.enabled);
    assert_eq!(
        custom.base_url.as_deref(),
        Some("https://custom-llm.example.com")
    );
    assert_eq!(custom.headers.get("X-API-Version").unwrap(), "2024-01");
}

#[test]
fn test_empty_api_key_fails_validation() {
    let yaml = r#"
credentials:
  - id: bad-cred
    provider: openai
    api_key: ""
"#;
    let result = GatewayConfig::from_yaml(yaml);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty API key"));
}

#[test]
fn test_empty_provider_fails_validation() {
    let yaml = r#"
credentials:
  - id: bad-cred
    provider: ""
    api_key: some-key
"#;
    let result = GatewayConfig::from_yaml(yaml);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty provider"));
}

// --- Trust Proxy Headers Tests ---

#[test]
fn test_trust_proxy_headers_default_false() {
    let config = GatewayConfig::default();
    assert!(!config.server.trust_proxy_headers);
}

#[test]
fn test_trust_proxy_headers_from_yaml() {
    let yaml = r"
server:
  trust_proxy_headers: true
";
    let config = GatewayConfig::from_yaml(yaml).unwrap();
    assert!(config.server.trust_proxy_headers);
}

// --- Provider Configuration Tests ---

#[test]
fn test_is_provider_enabled_default() {
    let config = GatewayConfig::default();
    // Unknown provider should be enabled by default
    assert!(config.is_provider_enabled("unknown-provider"));
    assert!(config.is_provider_enabled("openai"));
    assert!(config.is_provider_enabled("google"));
}

#[test]
fn test_is_provider_enabled_explicitly_disabled() {
    let yaml = r"
providers:
  openai:
    enabled: false
";
    let config = GatewayConfig::from_yaml(yaml).unwrap();
    assert!(!config.is_provider_enabled("openai"));
    // Other providers still default to enabled
    assert!(config.is_provider_enabled("google"));
}

#[test]
fn test_is_provider_enabled_explicitly_enabled() {
    let yaml = r"
providers:
  google:
    enabled: true
";
    let config = GatewayConfig::from_yaml(yaml).unwrap();
    assert!(config.is_provider_enabled("google"));
}
