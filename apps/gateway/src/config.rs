//! Configuration system for the LLM Gateway
//!
//! Handles loading and validating gateway configuration from YAML files
//! and environment variables.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
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
        for provider in self.providers.values_mut() {
            if let Some(ref mut base_url) = provider.base_url {
                *base_url = expand_env_var(base_url);
            }
            for value in provider.headers.values_mut() {
                *value = expand_env_var(value);
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

        // Validate provider base URLs (SSRF protection)
        for cred in &self.credentials {
            if let Some(ref base_url) = cred.base_url {
                validate_url_not_private(base_url)
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

    /// Check if authentication is enabled (at least one auth token configured)
    pub fn is_auth_enabled(&self) -> bool {
        !self.server.auth_tokens.is_empty()
    }
}

/// Expand environment variable references in a string
/// Supports ${VAR_NAME}, ${VAR_NAME:-default}, and embedded references
/// e.g., "Bearer ${AUTH_KEY}" or "${HOST:-localhost}:${PORT}"
fn expand_env_var(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut rest = value;

    while let Some(start) = rest.find("${") {
        result.push_str(&rest[..start]);
        rest = &rest[start + 2..];

        if let Some(end) = rest.find('}') {
            let inner = &rest[..end];
            rest = &rest[end + 1..];

            let expanded = if let Some((var_name, default)) = inner.split_once(":-") {
                std::env::var(var_name).unwrap_or_else(|_| default.to_string())
            } else {
                std::env::var(inner).unwrap_or_default()
            };
            result.push_str(&expanded);
        } else {
            // No closing brace, treat as literal
            result.push_str("${");
        }
    }

    result.push_str(rest);
    result
}

/// Constant-time comparison of two token strings to prevent timing attacks.
/// Returns `true` if tokens are byte-equal, always comparing the full length
/// of both strings regardless of where they differ.
pub fn constant_time_token_eq(a: &str, b: &str) -> bool {
    use subtle::ConstantTimeEq;
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    a_bytes.ct_eq(b_bytes).into()
}

/// Constant-time check whether `token` matches any entry in `configured_tokens`.
/// Iterates over all configured tokens regardless of where a match occurs,
/// preventing timing side-channels from leaking token ordering or count.
pub fn constant_time_token_matches(token: &str, configured_tokens: &[String]) -> bool {
    use subtle::ConstantTimeEq;
    let token_bytes = token.as_bytes();
    let mut result: u8 = 0;
    for configured in configured_tokens {
        let eq = configured.as_bytes().ct_eq(token_bytes).unwrap_u8();
        result |= eq;
    }
    result != 0
}

/// Validate that a URL does not point to a private, loopback, link-local,
/// or cloud metadata address. Returns `Ok(())` if the URL is safe, or an
/// error describing the rejected address.
pub fn validate_url_not_private(url_str: &str) -> Result<()> {
    let parsed = url::Url::parse(url_str).with_context(|| format!("Invalid URL: {url_str}"))?;

    let host = parsed
        .host()
        .ok_or_else(|| anyhow::anyhow!("URL has no host: {url_str}"))?;

    let ip = match host {
        url::Host::Domain(_) => return Ok(()), // Domain names are allowed (DNS rebinding is separate concern)
        url::Host::Ipv4(v4) => IpAddr::V4(v4),
        url::Host::Ipv6(v6) => IpAddr::V6(v6),
    };

    if is_private_ip(&ip) {
        anyhow::bail!(
            "URL points to a private/internal IP address ({ip}), which is not allowed (SSRF protection)"
        );
    }

    Ok(())
}

/// Check whether an IP address falls into a private, loopback, link-local,
/// or cloud metadata range.
fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            // Use stdlib methods for standard private/loopback/link-local ranges
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                // Zero network: 0.0.0.0/8
                || octets[0] == 0
                // IETF Protocol Assignments (192.0.0.0/24) and TEST-NET-1 (192.0.2.0/24)
                || (octets[0] == 192
                    && octets[1] == 0
                    && (octets[2] == 0 || octets[2] == 2))
                // TEST-NET-2 (198.51.100.0/24)
                || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
                // TEST-NET-3 (203.0.113.0/24)
                || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
                // Reserved for Future Use (includes broadcast): 255.0.0.0/8
                || octets[0] == 255
        },
        IpAddr::V6(v6) => {
            // IPv4-mapped IPv6 (::ffff:x.x.x.x) — e.g. ::ffff:127.0.0.1 bypasses
            // pure-IPv4 checks, so we unwrap the embedded IPv4 address and re-check.
            if let Some(mapped) = is_ipv4_mapped(v6) {
                return is_private_ip(&IpAddr::V4(mapped));
            }

            // IPv4-compatible IPv6 (::/96) — deprecated but still routable in some stacks.
            // ipv6_to_v4_compat handles the unspecified address (::) exclusion internally.
            if let Some(v4) = ipv6_to_v4_compat(v6) {
                return is_private_ip(&IpAddr::V4(v4));
            }

            let segments = v6.segments();
            // Loopback: ::1
            v6.is_loopback()
                // Private: fc00::/7 (unique local)
                || (0xfc00..=0xfdff).contains(&segments[0])
                // Link-local: fe80::/10
                || (0xfe80..=0xfebf).contains(&segments[0])
                // Unspecified: ::
                || v6.is_unspecified()
                // Multicast: ff00::/8
                || segments[0] >= 0xff00
        },
    }
}

/// Extract the embedded IPv4 address from an IPv4-mapped IPv6 address (::ffff:x.x.x.x).
fn is_ipv4_mapped(v6: &std::net::Ipv6Addr) -> Option<std::net::Ipv4Addr> {
    let segments = v6.segments();
    if segments[0..6] == [0, 0, 0, 0, 0, 0xffff] {
        Some(std::net::Ipv4Addr::new(
            (segments[6] >> 8) as u8,
            segments[6] as u8,
            (segments[7] >> 8) as u8,
            segments[7] as u8,
        ))
    } else {
        None
    }
}

/// Extract the embedded IPv4 address from an IPv4-compatible IPv6 address (::x.x.x.x).
/// Only extracts when the low 32 bits are non-zero (i.e., not the unspecified address ::).
fn ipv6_to_v4_compat(v6: &std::net::Ipv6Addr) -> Option<std::net::Ipv4Addr> {
    let segments = v6.segments();
    if segments[0..6] == [0, 0, 0, 0, 0, 0] && (segments[6] != 0 || segments[7] != 0) {
        Some(std::net::Ipv4Addr::new(
            (segments[6] >> 8) as u8,
            segments[6] as u8,
            (segments[7] >> 8) as u8,
            segments[7] as u8,
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_constant_time_token_eq_same_tokens() {
        assert!(constant_time_token_eq(
            "secret-token-123",
            "secret-token-123"
        ));
    }

    #[test]
    fn test_constant_time_token_eq_different_tokens_same_length() {
        assert!(!constant_time_token_eq(
            "secret-token-123",
            "secret-token-124"
        ));
    }

    #[test]
    fn test_constant_time_token_eq_different_lengths() {
        assert!(!constant_time_token_eq("short", "much-longer-token"));
    }

    #[test]
    fn test_constant_time_token_eq_empty_tokens() {
        assert!(constant_time_token_eq("", ""));
    }

    #[test]
    fn test_constant_time_token_eq_one_empty() {
        assert!(!constant_time_token_eq("nonempty", ""));
    }

    #[test]
    fn test_constant_time_token_matches_hit() {
        let tokens = vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];
        assert!(constant_time_token_matches("beta", &tokens));
    }

    #[test]
    fn test_constant_time_token_matches_miss() {
        let tokens = vec!["alpha".to_string(), "beta".to_string()];
        assert!(!constant_time_token_matches("delta", &tokens));
    }

    #[test]
    fn test_constant_time_token_matches_empty_list() {
        assert!(!constant_time_token_matches("anything", &[]));
    }

    #[test]
    fn test_constant_time_token_matches_first() {
        let tokens = vec!["first".to_string(), "second".to_string()];
        assert!(constant_time_token_matches("first", &tokens));
    }

    #[test]
    fn test_constant_time_token_matches_last() {
        let tokens = vec!["first".to_string(), "last".to_string()];
        assert!(constant_time_token_matches("last", &tokens));
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

    // --- SSRF protection tests ---

    #[test]
    fn test_ssrf_reject_loopback() {
        let yaml = r#"
credentials:
  - id: test
    provider: openai
    api_key: key1
    base_url: http://127.0.0.1:8000
"#;
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
            r#"
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
"#,
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
    fn test_ssrf_reject_loopback_localhost() {
        // localhost resolves to 127.0.0.1, but since it's not a literal IP
        // our parser allows it (DNS rebinding is out of scope).
        // This test verifies the IP-based check path.
        let result = validate_url_not_private("http://127.0.0.1/v1/chat");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_private_10_range() {
        let result = validate_url_not_private("http://10.0.0.1/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_private_172_range() {
        let result = validate_url_not_private("http://172.16.0.1/api");
        assert!(result.is_err());
        let result = validate_url_not_private("http://172.31.255.255/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_private_192_range() {
        let result = validate_url_not_private("http://192.168.1.1/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_link_local() {
        let result = validate_url_not_private("http://169.254.169.254/latest/meta-data/");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_ipv6_loopback() {
        let result = validate_url_not_private("http://[::1]:8000/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_ipv6_unique_local() {
        let result = validate_url_not_private("http://[fc00::1]/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_ipv6_link_local() {
        let result = validate_url_not_private("http://[fe80::1]/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_allow_public_ip() {
        let result = validate_url_not_private("https://api.openai.com/v1/chat/completions");
        // openai.com is a domain, not a literal IP — allowed
        assert!(result.is_ok());
    }

    #[test]
    fn test_ssrf_allow_public_literal_ip() {
        let result = validate_url_not_private("https://1.1.1.1/api");
        assert!(result.is_ok());
    }

    #[test]
    fn test_ssrf_allow_credential_without_base_url() {
        let yaml = r#"
credentials:
  - id: test
    provider: openai
    api_key: key1
"#;
        let result = GatewayConfig::from_yaml(yaml);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ssrf_reject_zero_network() {
        let result = validate_url_not_private("http://0.0.0.0/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_ipv4_mapped_ipv6_loopback() {
        let result = validate_url_not_private("http://[::ffff:127.0.0.1]:8000/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_ipv4_mapped_ipv6_cloud_metadata() {
        let result = validate_url_not_private("http://[::ffff:169.254.169.254]/latest/meta-data/");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_ipv4_mapped_ipv6_private_10() {
        let result = validate_url_not_private("http://[::ffff:10.0.0.1]/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_ipv4_mapped_ipv6_private_192() {
        let result = validate_url_not_private("http://[::ffff:192.168.1.1]/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_allow_ipv4_mapped_public_ip() {
        let result = validate_url_not_private("http://[::ffff:1.1.1.1]/api");
        assert!(result.is_ok());
    }

    #[test]
    fn test_ssrf_reject_ietf_protocol_assignments() {
        let result = validate_url_not_private("http://192.0.0.1/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_test_net_1() {
        let result = validate_url_not_private("http://192.0.2.1/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_test_net_2() {
        let result = validate_url_not_private("http://198.51.100.1/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_test_net_3() {
        let result = validate_url_not_private("http://203.0.113.1/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_broadcast() {
        let result = validate_url_not_private("http://255.255.255.255/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_ipv6_unspecified() {
        let result = validate_url_not_private("http://[::]/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_ipv6_multicast() {
        let result = validate_url_not_private("http://[ff02::1]/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_ipv4_compatible_loopback() {
        let result = validate_url_not_private("http://[::127.0.0.1]/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_reject_ipv4_compatible_private() {
        let result = validate_url_not_private("http://[::192.168.1.1]/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_ssrf_allow_public_ipv4_literal() {
        let result = validate_url_not_private("https://8.8.8.8/api");
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
    fn test_provider_env_vars_with_set_variables() {
        std::env::set_var("TEST_PROVIDER_URL", "https://custom.provider.com");
        std::env::set_var("TEST_PROVIDER_KEY", "secret-key");

        let config = GatewayConfig::from_yaml(
            r#"
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
"#,
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

    #[test]
    fn test_provider_config_without_env_vars_unchanged() {
        let config = GatewayConfig::from_yaml(
            r#"
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
"#,
        )
        .unwrap();

        let openai = config.providers.get("openai").unwrap();
        assert_eq!(openai.base_url.as_deref(), Some("https://api.openai.com"));
        assert_eq!(openai.headers.get("X-Custom").unwrap(), "literal-value");
    }

    // --- Config File Integration Tests ---

    #[test]
    fn test_from_file_valid_yaml() {
        let yaml_content = r#"
server:
  port: 9090
  host: 127.0.0.1
  timeout_secs: 60
credentials:
  - id: openai-prod
    provider: openai
    api_key: sk-test-key-123 # gitleaks:allow
    priority: 5
"#;
        let mut tmp_file = NamedTempFile::new().expect("failed to create temp file");
        write!(tmp_file, "{}", yaml_content).expect("failed to write temp file");
        let config =
            GatewayConfig::from_file(tmp_file.path()).expect("failed to load config from file");

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
        let yaml_content = r#"
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
"#;
        let mut tmp_file = NamedTempFile::new().expect("failed to create temp file");
        write!(tmp_file, "{}", yaml_content).expect("failed to write temp file");
        let config =
            GatewayConfig::from_file(tmp_file.path()).expect("failed to load config from file");

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
        let yaml_content = r#"
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
"#;
        let mut tmp_file = NamedTempFile::new().expect("failed to create temp file");
        write!(tmp_file, "{}", yaml_content).expect("failed to write temp file");
        let config =
            GatewayConfig::from_file(tmp_file.path()).expect("failed to load config from file");

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
        let mut tmp_file = NamedTempFile::new().expect("failed to create temp file");
        write!(tmp_file, "{}", yaml_content).expect("failed to write temp file");
        let config =
            GatewayConfig::from_file(tmp_file.path()).expect("failed to load config from file");

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
        let yaml = r#"
server:
  trust_proxy_headers: true
"#;
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
        let yaml = r#"
providers:
  openai:
    enabled: false
"#;
        let config = GatewayConfig::from_yaml(yaml).unwrap();
        assert!(!config.is_provider_enabled("openai"));
        // Other providers still default to enabled
        assert!(config.is_provider_enabled("google"));
    }

    #[test]
    fn test_is_provider_enabled_explicitly_enabled() {
        let yaml = r#"
providers:
  google:
    enabled: true
"#;
        let config = GatewayConfig::from_yaml(yaml).unwrap();
        assert!(config.is_provider_enabled("google"));
    }
}
