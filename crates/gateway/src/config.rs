//! Configuration system for the LLM Gateway
//!
//! Handles loading and validating gateway configuration from YAML files
//! and environment variables.

use anyhow::{Context, Result};
use gateway_utils::{expand_env_var, validate_url_not_private};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub use gateway_utils::constant_time_token_matches;

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

    /// Whether to trust X-Forwarded-For / X-Real-IP headers for rate limiting.
    /// Only enable when the gateway is behind a trusted reverse proxy.
    /// When false (default), all requests share a single rate-limit bucket,
    /// preventing header-spoofing bypasses.
    #[serde(default)]
    pub trust_proxy_headers: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            host: default_host(),
            timeout_secs: default_timeout(),
            auth_tokens: Vec::new(),
            trust_proxy_headers: false,
        }
    }
}

const fn default_port() -> u16 {
    3000
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

const fn default_timeout() -> u64 {
    120
}

/// Credential configuration for a provider API key
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CredentialConfig {
    /// Unique credential identifier
    pub id: String,

    /// Provider name (e.g., "anthropic", "openai")
    pub provider: String,

    /// API key (can be loaded from env with ${`VAR_NAME`})
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
    /// Selection strategy: "weighted", "`time_aware`", "`quota_aware`", "adaptive", "`policy_aware`"
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

const fn default_true() -> bool {
    true
}

const fn default_min_healthy() -> usize {
    1
}

const fn default_fallback_depth() -> usize {
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
    /// Load configuration from a YAML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or the YAML content
    /// is invalid (including failed env-var expansion or validation).
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .with_context(|| "Failed to read configuration file")?;

        Self::from_yaml(&content)
    }

    /// Parse configuration from a YAML string.
    ///
    /// # Errors
    ///
    /// Returns an error if the YAML is malformed, environment variable
    /// expansion fails, or configuration validation fails.
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let mut config: Self =
            serde_yaml_ng::from_str(yaml).with_context(|| "Failed to parse YAML configuration")?;

        // Expand environment variables in secrets
        config.expand_env_vars();

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Expand environment variable references in secrets.
    fn expand_env_vars(&mut self) {
        for cred in &mut self.credentials {
            cred.api_key = expand_env_var(&cred.api_key);
            if let Some(ref mut base_url) = cred.base_url {
                *base_url = expand_env_var(base_url);
            }
        }
        for provider in self.providers.values_mut() {
            if let Some(ref mut base_url) = provider.base_url {
                *base_url = expand_env_var(base_url);
            }
            for value in provider.headers.values_mut() {
                *value = expand_env_var(value);
            }
        }
    }

    /// Validate the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if credential IDs are duplicated, API keys or
    /// providers are empty, base URLs point to private hosts, or the
    /// routing strategy is not one of the recognized values.
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

        // Validate provider base URLs (SSRF protection)
        for cred in &self.credentials {
            if let Some(ref base_url) = cred.base_url {
                validate_url_not_private(base_url)
                    .map_err(anyhow::Error::from)
                    .with_context(|| format!("Credential {} has an invalid base_url", cred.id))?;
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

    /// Get credentials for a specific provider.
    #[must_use]
    pub fn credentials_for_provider(&self, provider: &str) -> Vec<&CredentialConfig> {
        self.credentials
            .iter()
            .filter(|c| c.provider == provider)
            .collect()
    }

    /// Check if a provider is enabled.
    #[must_use]
    pub fn is_provider_enabled(&self, provider: &str) -> bool {
        self.providers.get(provider).is_none_or(|p| p.enabled) // Enabled by default if not configured
    }

    /// Check if authentication is enabled (at least one auth token configured).
    #[must_use]
    pub fn is_auth_enabled(&self) -> bool {
        !self.server.auth_tokens.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_env_vars_with_set_variables() {
        std::env::set_var("TEST_PROVIDER_URL", "https://custom.provider.com");
        std::env::set_var("TEST_PROVIDER_KEY", "secret-key");

        let config = GatewayConfig::from_yaml(
            r"
credentials:
  - id: cred1
    provider: openai
    api_key: key1
providers:
  openai:
    enabled: true
    base_url: ${TEST_PROVIDER_URL}
    headers:
      X-Api-Key: ${TEST_PROVIDER_KEY}
",
        )
        .unwrap();

        let openai = config.providers.get("openai").unwrap();
        assert_eq!(
            openai.base_url.as_deref(),
            Some("https://custom.provider.com")
        );
        assert_eq!(openai.headers.get("X-Api-Key").unwrap(), "secret-key");

        std::env::remove_var("TEST_PROVIDER_URL");
        std::env::remove_var("TEST_PROVIDER_KEY");
    }
}
