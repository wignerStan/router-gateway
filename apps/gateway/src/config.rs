//! Configuration system for the LLM Gateway
//!
//! Handles loading and validating gateway configuration from YAML files
//! and environment variables.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Main gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GatewayConfig {
    /// Server configuration
    #[serde(default)]
    pub server: ServerConfig,

    /// Credential definitions
    #[serde(default)]
    pub credentials: Vec<CredentialConfig>,

    /// Routing policy configuration
    #[serde(default)]
    pub routing: RoutingPolicyConfig,

    /// Provider-specific settings
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

/// Server (HTTP) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Port to listen on
    #[serde(default = "default_port")]
    pub port: u16,

    /// Host to bind to
    #[serde(default = "default_host")]
    pub host: String,

    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    /// Authentication tokens for API access (Bearer tokens)
    /// If empty, authentication is disabled (not recommended for production)
    #[serde(default)]
    pub auth_tokens: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            host: default_host(),
            timeout_secs: default_timeout(),
            auth_tokens: Vec::new(),
        }
    }
}

fn default_port() -> u16 {
    3000
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_timeout() -> u64 {
    120
}

/// Credential configuration for a provider API key
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CredentialConfig {
    /// Unique credential identifier
    pub id: String,

    /// Provider name (e.g., "anthropic", "openai")
    pub provider: String,

    /// API key (can be loaded from env with ${VAR_NAME})
    pub api_key: String,

    /// Optional base URL override
    #[serde(default)]
    pub base_url: Option<String>,

    /// Optional organization ID
    #[serde(default)]
    pub organization: Option<String>,

    /// Models this credential can access (empty = all)
    #[serde(default)]
    pub allowed_models: Vec<String>,

    /// Priority weight (higher = preferred)
    #[serde(default)]
    pub priority: i32,

    /// Daily request quota
    #[serde(default)]
    pub daily_quota: Option<u64>,

    /// Per-minute request limit
    #[serde(default)]
    pub rate_limit: Option<u64>,
}

/// Routing policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingPolicyConfig {
    /// Selection strategy: "weighted", "time_aware", "quota_aware", "adaptive", "policy_aware"
    #[serde(default = "default_strategy")]
    pub strategy: String,

    /// Enable session affinity for multi-turn conversations
    #[serde(default = "default_true")]
    pub session_affinity: bool,

    /// Minimum healthy credentials required
    #[serde(default = "default_min_healthy")]
    pub min_healthy_credentials: usize,

    /// Fallback chain depth
    #[serde(default = "default_fallback_depth")]
    pub fallback_depth: usize,
}

impl Default for RoutingPolicyConfig {
    fn default() -> Self {
        Self {
            strategy: default_strategy(),
            session_affinity: default_true(),
            min_healthy_credentials: default_min_healthy(),
            fallback_depth: default_fallback_depth(),
        }
    }
}

fn default_strategy() -> String {
    "weighted".to_string()
}

fn default_true() -> bool {
    true
}

fn default_min_healthy() -> usize {
    1
}

fn default_fallback_depth() -> usize {
    2
}

/// Provider-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Enable/disable this provider
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Default base URL for this provider
    #[serde(default)]
    pub base_url: Option<String>,

    /// Default timeout override
    #[serde(default)]
    pub timeout_secs: Option<u64>,

    /// Additional headers to send
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

impl GatewayConfig {
    /// Load configuration from a YAML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .with_context(|| "Failed to read configuration file")?;

        Self::from_yaml(&content)
    }

    /// Parse configuration from YAML string
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let mut config: GatewayConfig =
            serde_yaml::from_str(yaml).with_context(|| "Failed to parse YAML configuration")?;

        // Expand environment variables in secrets
        config.expand_env_vars()?;

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Expand environment variable references in secrets
    fn expand_env_vars(&mut self) -> Result<()> {
        for cred in &mut self.credentials {
            cred.api_key = expand_env_var(&cred.api_key);
            if let Some(ref mut base_url) = cred.base_url {
                *base_url = expand_env_var(base_url);
            }
        }
        Ok(())
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Check for duplicate credential IDs
        let mut seen_ids = std::collections::HashSet::new();
        for cred in &self.credentials {
            if !seen_ids.insert(&cred.id) {
                anyhow::bail!("Duplicate credential ID: {}", cred.id);
            }

            if cred.api_key.is_empty() {
                anyhow::bail!("Credential {} has empty API key", cred.id);
            }

            if cred.provider.is_empty() {
                anyhow::bail!("Credential {} has empty provider", cred.id);
            }
        }

        // Validate routing strategy - must match smart-routing strategies
        let valid_strategies = [
            "weighted",
            "time_aware",
            "quota_aware",
            "adaptive",
            "policy_aware",
        ];
        if !valid_strategies.contains(&self.routing.strategy.as_str()) {
            anyhow::bail!(
                "Invalid routing strategy: {}. Valid options: {:?}",
                self.routing.strategy,
                valid_strategies
            );
        }

        Ok(())
    }

    /// Get credentials for a specific provider
    pub fn credentials_for_provider(&self, provider: &str) -> Vec<&CredentialConfig> {
        self.credentials
            .iter()
            .filter(|c| c.provider == provider)
            .collect()
    }

    /// Check if a provider is enabled
    pub fn is_provider_enabled(&self, provider: &str) -> bool {
        self.providers
            .get(provider)
            .map(|p| p.enabled)
            .unwrap_or(true) // Enabled by default if not configured
    }
}

/// Expand a single environment variable reference
/// Supports ${VAR_NAME} and ${VAR_NAME:-default} syntax
fn expand_env_var(value: &str) -> String {
    if let Some(stripped) = value.strip_prefix("${").and_then(|s| s.strip_suffix("}")) {
        // Check for default value syntax
        if let Some((var_name, default)) = stripped.split_once(":-") {
            std::env::var(var_name).unwrap_or_else(|_| default.to_string())
        } else {
            // Return empty string for unset env vars so validation catches them
            std::env::var(stripped).unwrap_or_default()
        }
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GatewayConfig::default();
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.server.host, "0.0.0.0");
        assert!(config.credentials.is_empty());
    }

    #[test]
    fn test_parse_minimal_yaml() {
        let yaml = r#"
server:
  port: 8080
"#;
        let config = GatewayConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.server.port, 8080);
    }

    #[test]
    fn test_parse_credentials() {
        let yaml = r#"
credentials:
  - id: anthropic-primary
    provider: anthropic
    api_key: sk-test-key
    priority: 10
"#;
        let config = GatewayConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.credentials.len(), 1);
        assert_eq!(config.credentials[0].id, "anthropic-primary");
        assert_eq!(config.credentials[0].provider, "anthropic");
        assert_eq!(config.credentials[0].priority, 10);
    }

    #[test]
    fn test_duplicate_credential_id_fails() {
        let yaml = r#"
credentials:
  - id: test-cred
    provider: anthropic
    api_key: key1
  - id: test-cred
    provider: openai
    api_key: key2
"#;
        let result = GatewayConfig::from_yaml(yaml);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Duplicate credential ID"));
    }

    #[test]
    fn test_invalid_strategy_fails() {
        let yaml = r#"
routing:
  strategy: invalid
"#;
        let result = GatewayConfig::from_yaml(yaml);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid routing strategy"));
    }

    #[test]
    fn test_expand_env_var() {
        std::env::set_var("TEST_API_KEY", "secret-value");
        let expanded = expand_env_var("${TEST_API_KEY}");
        assert_eq!(expanded, "secret-value");

        let with_default = expand_env_var("${NONEXISTENT_VAR:-default-value}");
        assert_eq!(with_default, "default-value");

        let literal = expand_env_var("literal-value");
        assert_eq!(literal, "literal-value");

        std::env::remove_var("TEST_API_KEY");
    }

    #[test]
    fn test_provider_filtering() {
        let config = GatewayConfig::from_yaml(
            r#"
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
"#,
        )
        .unwrap();

        let anthropic_creds = config.credentials_for_provider("anthropic");
        assert_eq!(anthropic_creds.len(), 2);

        let openai_creds = config.credentials_for_provider("openai");
        assert_eq!(openai_creds.len(), 1);
    }
}
