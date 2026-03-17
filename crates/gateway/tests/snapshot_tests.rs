#![allow(missing_docs, clippy::expect_used)]

use gateway::config::{GatewayConfig, RoutingPolicyConfig, ServerConfig};
use gateway::state::{HealthStatus, ModelInfo};
use serde_json::json;
use std::collections::BTreeMap;

/// Convert a `GatewayConfig`'s providers `HashMap` to a `serde_json::Value`
/// with sorted keys for deterministic snapshot output.
fn config_to_sorted_value(config: &GatewayConfig) -> serde_json::Value {
    let mut value = serde_json::to_value(config).expect("GatewayConfig should serialize to JSON");
    if let Some(providers) = value.get_mut("providers").and_then(|v| v.as_object_mut()) {
        let sorted: BTreeMap<String, serde_json::Value> = providers.clone().into_iter().collect();
        *providers = serde_json::Map::from_iter(sorted);
    }
    value
}

// ===================================================================
// Config Serialization Snapshots
// ===================================================================

#[test]
fn snapshot_default_config() {
    let config = GatewayConfig::default();
    insta::assert_yaml_snapshot!(config, {
        ".server.host" => "[host]",
    });
}

#[test]
fn snapshot_default_server_config() {
    let config = ServerConfig::default();
    insta::assert_yaml_snapshot!(config, {
        ".host" => "[host]",
    });
}

#[test]
fn snapshot_default_routing_policy() {
    let config = RoutingPolicyConfig::default();
    insta::assert_yaml_snapshot!(config);
}

#[test]
fn snapshot_minimal_yaml_config() {
    let yaml = r"
server:
  port: 8080
";
    let config = GatewayConfig::from_yaml(yaml).expect("failed to parse minimal YAML");
    insta::assert_yaml_snapshot!(config, {
        ".server.host" => "[host]",
    });
}

#[test]
fn snapshot_config_with_credentials() {
    let yaml = r"
credentials:
  - id: anthropic-primary
    provider: anthropic
    api_key: sk-test-key
    priority: 10
  - id: openai-backup
    provider: openai
    api_key: sk-openai-key
    priority: 5
    allowed_models:
      - gpt-4
      - gpt-3.5-turbo
";
    let config = GatewayConfig::from_yaml(yaml).expect("failed to parse credentials YAML");
    insta::assert_yaml_snapshot!(config, {
        ".server.host" => "[host]",
    });
}

#[test]
fn snapshot_full_config_all_sections() {
    let yaml = r"
server:
  port: 8080
  timeout_secs: 90
  auth_tokens:
    - token-one
    - token-two
  trust_proxy_headers: true

credentials:
  - id: openai-primary
    provider: openai
    api_key: sk-openai-key
    priority: 10
    allowed_models:
      - gpt-4
      - gpt-3.5-turbo
    daily_quota: 10000
    rate_limit: 60
  - id: google-primary
    provider: google
    api_key: google-key-123
    priority: 8
    base_url: https://generativelanguage.googleapis.com/v1beta

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
    let config = GatewayConfig::from_yaml(yaml).expect("failed to parse full config YAML");
    insta::assert_yaml_snapshot!(config_to_sorted_value(&config), {
        ".server.host" => "[host]",
    });
}

#[test]
fn snapshot_credential_with_all_fields() {
    let yaml = r"
credentials:
  - id: full-cred
    provider: openai
    api_key: sk-full-test-key
    base_url: https://custom.openai.proxy.com
    organization: org-example-123
    allowed_models:
      - gpt-4
      - gpt-4-turbo
    priority: 15
    daily_quota: 10000
    rate_limit: 60
";
    let config = GatewayConfig::from_yaml(yaml).expect("failed to parse credential YAML");
    let cred = &config.credentials[0];
    insta::assert_yaml_snapshot!(cred);
}

#[test]
fn snapshot_credential_minimal_fields() {
    let yaml = r"
credentials:
  - id: minimal-cred
    provider: anthropic
    api_key: sk-minimal
";
    let config = GatewayConfig::from_yaml(yaml).expect("failed to parse minimal credential YAML");
    let cred = &config.credentials[0];
    insta::assert_yaml_snapshot!(cred);
}

#[test]
fn snapshot_multiple_providers_config() {
    let yaml = r#"
credentials:
  - id: c1
    provider: openai
    api_key: key1
  - id: c2
    provider: google
    api_key: key2
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
    let config = GatewayConfig::from_yaml(yaml).expect("failed to parse providers YAML");
    // Convert to BTreeMap for deterministic key ordering in snapshots
    let sorted_providers: BTreeMap<_, _> = config.providers.into_iter().collect();
    insta::assert_yaml_snapshot!(sorted_providers);
}

#[test]
fn snapshot_routing_policy_weighted() {
    let config = GatewayConfig::from_yaml("routing:\n  strategy: weighted\n")
        .expect("failed to parse routing YAML");
    insta::assert_yaml_snapshot!(config.routing);
}

#[test]
fn snapshot_routing_policy_adaptive() {
    let config = GatewayConfig::from_yaml("routing:\n  strategy: adaptive\n")
        .expect("failed to parse routing YAML");
    insta::assert_yaml_snapshot!(config.routing);
}

#[test]
fn snapshot_routing_policy_time_aware() {
    let config = GatewayConfig::from_yaml("routing:\n  strategy: time_aware\n")
        .expect("failed to parse routing YAML");
    insta::assert_yaml_snapshot!(config.routing);
}

#[test]
fn snapshot_routing_policy_quota_aware() {
    let config = GatewayConfig::from_yaml("routing:\n  strategy: quota_aware\n")
        .expect("failed to parse routing YAML");
    insta::assert_yaml_snapshot!(config.routing);
}

#[test]
fn snapshot_routing_policy_policy_aware() {
    let config = GatewayConfig::from_yaml("routing:\n  strategy: policy_aware\n")
        .expect("failed to parse routing YAML");
    insta::assert_yaml_snapshot!(config.routing);
}

// ===================================================================
// State / Response Type Snapshots
// ===================================================================

#[test]
fn snapshot_health_status_healthy() {
    let status = HealthStatus {
        status: "healthy".to_string(),
        uptime_secs: 3600,
        credential_count: 3,
        healthy_count: 3,
        degraded_count: 0,
        unhealthy_count: 0,
    };
    insta::assert_yaml_snapshot!(status, {
        ".uptime_secs" => "[uptime]",
    });
}

#[test]
fn snapshot_health_status_degraded() {
    let status = HealthStatus {
        status: "degraded".to_string(),
        uptime_secs: 7200,
        credential_count: 4,
        healthy_count: 2,
        degraded_count: 1,
        unhealthy_count: 1,
    };
    insta::assert_yaml_snapshot!(status, {
        ".uptime_secs" => "[uptime]",
    });
}

#[test]
fn snapshot_health_status_no_credentials() {
    let status = HealthStatus {
        status: "healthy".to_string(),
        uptime_secs: 0,
        credential_count: 0,
        healthy_count: 0,
        degraded_count: 0,
        unhealthy_count: 0,
    };
    insta::assert_yaml_snapshot!(status, {
        ".uptime_secs" => "[uptime]",
    });
}

#[test]
fn snapshot_model_info_with_capabilities() {
    let model = ModelInfo {
        id: "gpt-4".to_string(),
        provider: "openai".to_string(),
        capabilities: vec![
            "streaming".to_string(),
            "tools".to_string(),
            "vision".to_string(),
        ],
        context_window: 128_000,
    };
    insta::assert_yaml_snapshot!(model);
}

#[test]
fn snapshot_model_info_wildcard() {
    let model = ModelInfo {
        id: "openai:*".to_string(),
        provider: "openai".to_string(),
        capabilities: vec!["all".to_string()],
        context_window: 128_000,
    };
    insta::assert_yaml_snapshot!(model);
}

#[test]
fn snapshot_model_info_minimal() {
    let model = ModelInfo {
        id: "claude-3-opus".to_string(),
        provider: "anthropic".to_string(),
        capabilities: vec![],
        context_window: 200_000,
    };
    insta::assert_yaml_snapshot!(model);
}

// ===================================================================
// Route Response Structure Snapshots
// ===================================================================

#[test]
fn snapshot_root_response_structure() {
    // Snapshot the static root endpoint JSON structure
    let response = json!({
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
    });
    insta::assert_yaml_snapshot!(response);
}

#[test]
fn snapshot_error_response_no_route() {
    let error = json!({
        "error": {
            "type": "no_route_available",
            "message": "No suitable routes found. Configure credentials in gateway.yaml"
        }
    });
    insta::assert_yaml_snapshot!(error);
}

#[test]
fn snapshot_error_response_rate_limit() {
    let error = json!({
        "error": {
            "type": "rate_limit_error",
            "message": "Too many requests. Please try again later."
        }
    });
    insta::assert_yaml_snapshot!(error);
}

#[test]
fn snapshot_error_response_unauthorized() {
    let error = json!({
        "error": {
            "type": "invalid_request_error",
            "message": "Invalid or expired API token"
        }
    });
    insta::assert_yaml_snapshot!(error);
}

#[test]
fn snapshot_error_response_missing_auth() {
    let error = json!({
        "error": {
            "type": "invalid_request_error",
            "message": "Missing Authorization header. Use: Authorization: Bearer <token>"
        }
    });
    insta::assert_yaml_snapshot!(error);
}

#[test]
fn snapshot_error_response_config_error() {
    let error = json!({
        "error": {
            "type": "config_error",
            "message": "Gateway is improperly configured: No authentication tokens available."
        }
    });
    insta::assert_yaml_snapshot!(error);
}

#[test]
fn snapshot_models_response_empty() {
    let response = json!({
        "models": [],
        "count": 0,
        "message": "No models configured. Add credentials to gateway.yaml"
    });
    insta::assert_yaml_snapshot!(response);
}

#[test]
fn snapshot_models_response_with_credentials() {
    let response = json!({
        "models": [
            {
                "id": "gpt-4",
                "provider": "openai",
                "capabilities": [],
                "context_window": 128_000
            },
            {
                "id": "gpt-3.5-turbo",
                "provider": "openai",
                "capabilities": [],
                "context_window": 128_000
            },
            {
                "id": "claude-3-opus",
                "provider": "anthropic",
                "capabilities": [],
                "context_window": 128_000
            }
        ],
        "count": 3,
        "message": "Models loaded from configuration"
    });
    insta::assert_yaml_snapshot!(response);
}

#[test]
fn snapshot_classification_output_text_request() {
    let classification = json!({
        "required_capabilities": {
            "vision": false,
            "tools": false,
            "streaming": false,
            "thinking": false
        },
        "estimated_tokens": 15,
        "format": "Chat",
        "quality_preference": "Balanced"
    });
    insta::assert_yaml_snapshot!(classification, {
        ".estimated_tokens" => "[token_count]",
    });
}

#[test]
fn snapshot_classification_output_vision_request() {
    let classification = json!({
        "required_capabilities": {
            "vision": true,
            "tools": false,
            "streaming": true,
            "thinking": false
        },
        "estimated_tokens": 512,
        "format": "Chat",
        "quality_preference": "Balanced"
    });
    insta::assert_yaml_snapshot!(classification, {
        ".estimated_tokens" => "[token_count]",
    });
}

#[test]
fn snapshot_classification_output_tool_request() {
    let classification = json!({
        "required_capabilities": {
            "vision": false,
            "tools": true,
            "streaming": false,
            "thinking": false
        },
        "estimated_tokens": 256,
        "format": "Chat",
        "quality_preference": "Balanced"
    });
    insta::assert_yaml_snapshot!(classification, {
        ".estimated_tokens" => "[token_count]",
    });
}

// ===================================================================
// Chat Completion Response Structure Snapshot
// ===================================================================

#[test]
fn snapshot_chat_completion_response_structure() {
    let response = json!({
        "id": "chatcmpl-[uuid]",
        "object": "chat.completion",
        "created": 1_710_000_000,
        "model": "gpt-4",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "[Gateway mock response - route: openai-primary, provider: openai]"
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 50,
            "completion_tokens": 50,
            "total_tokens": 100
        },
        "_gateway": {
            "route": {
                "credential_id": "openai-primary",
                "provider": "openai",
                "utility": "[utility]"
            },
            "classification": {
                "format": "Chat",
                "capabilities": {
                    "vision": false,
                    "tools": false,
                    "streaming": false
                }
            }
        }
    });
    insta::assert_yaml_snapshot!(response, {
        ".id" => "[chat_id]",
        ".created" => "[timestamp]",
        "._gateway.route.utility" => "[utility]",
    });
}
