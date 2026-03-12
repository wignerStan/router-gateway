use crate::health::HealthManager;
use crate::metrics::MetricsCollector;
use crate::weight::{AuthInfo, WeightCalculator};
use std::collections::HashSet;

/// Fallback route with ordering information
#[derive(Debug, Clone)]
pub struct FallbackRoute {
    /// Auth credential ID for this fallback
    pub auth_id: String,
    /// Position in fallback chain (0 = primary)
    pub position: usize,
    /// Calculated weight for this auth
    pub weight: f64,
    /// Provider identifier (if available)
    pub provider: Option<String>,
}

/// Configuration for fallback planning
#[derive(Debug, Clone)]
pub struct FallbackConfig {
    /// Maximum number of fallback routes to generate
    pub max_fallbacks: usize,
    /// Minimum number of fallbacks (even with limited candidates)
    pub min_fallbacks: usize,
    /// Enable provider diversity in fallback chain
    pub enable_provider_diversity: bool,
    /// Prefer different providers for consecutive fallbacks
    pub prefer_diverse_providers: bool,
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            max_fallbacks: 5,
            min_fallbacks: 2,
            enable_provider_diversity: true,
            prefer_diverse_providers: true,
        }
    }
}

/// Fallback planner for generating ordered fallback routes
#[derive(Debug, Clone)]
pub struct FallbackPlanner {
    config: FallbackConfig,
}

impl FallbackPlanner {
    /// Create a new fallback planner with default config
    pub fn new() -> Self {
        Self {
            config: FallbackConfig::default(),
        }
    }

    /// Create a new fallback planner with custom config
    pub fn with_config(config: FallbackConfig) -> Self {
        Self { config }
    }

    /// Generate ordered fallback routes from available auths
    ///
    /// Returns a Vec of FallbackRoute ordered by:
    /// 1. Weight (descending) - higher weight = better candidate
    /// 2. Provider diversity (if enabled) - prefer different providers
    /// 3. Health status - prefer healthy > degraded > unhealthy
    ///
    /// # Arguments
    /// * `auths` - Available auth credentials
    /// * `primary_id` - The primary selected auth ID (will be first in list)
    /// * `calculator` - Weight calculator for scoring
    /// * `metrics` - Metrics collector for performance data
    /// * `health` - Health manager for status data
    pub async fn generate_fallbacks(
        &self,
        auths: Vec<AuthInfo>,
        primary_id: Option<String>,
        calculator: &dyn WeightCalculator,
        metrics: &MetricsCollector,
        health: &HealthManager,
    ) -> Vec<FallbackRoute> {
        if auths.is_empty() {
            return Vec::new();
        }

        // Calculate weights for all available auths
        let mut weighted_auths = self
            .calculate_weights(auths, calculator, metrics, health)
            .await;

        // Filter to only available (positive weight) auths
        weighted_auths.retain(|w| w.weight > 0.0);

        if weighted_auths.is_empty() {
            return Vec::new();
        }

        // Sort by weight (descending) - highest weight first (NaN-safe)
        weighted_auths.sort_by(|a, b| {
            b.weight
                .partial_cmp(&a.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Apply provider diversity if enabled
        if self.config.enable_provider_diversity {
            weighted_auths = self.apply_provider_diversity(weighted_auths);
        }

        // Ensure primary is first (if specified)
        let fallbacks = if let Some(primary) = primary_id {
            self.ensure_primary_first(weighted_auths, &primary)
        } else {
            weighted_auths
        };

        // Limit to max_fallbacks
        let fallbacks: Vec<_> = fallbacks
            .into_iter()
            .enumerate()
            .map(|(idx, w)| FallbackRoute {
                auth_id: w.id,
                position: idx,
                weight: w.weight,
                provider: w.provider,
            })
            .take(self.config.max_fallbacks)
            .collect();

        // Ensure min_fallbacks by padding with remaining candidates if needed
        if fallbacks.len() < self.config.min_fallbacks {
            // We already filtered by weight > 0, so if we don't have enough,
            // we return what we have (can't invent auths out of thin air)
        }

        fallbacks
    }

    /// Calculate weights for all auths
    async fn calculate_weights(
        &self,
        auths: Vec<AuthInfo>,
        calculator: &dyn WeightCalculator,
        metrics: &MetricsCollector,
        health: &HealthManager,
    ) -> Vec<WeightedAuth> {
        let mut weighted_auths = Vec::new();

        for auth in auths {
            // Skip unavailable auths
            if auth.unavailable {
                continue;
            }

            // Get metrics and health
            let auth_metrics = metrics.get_metrics(&auth.id).await;
            let auth_health = health.get_status(&auth.id).await;

            // Check availability
            let is_available = health.is_available(&auth.id).await;
            if !is_available {
                continue;
            }

            // Calculate weight
            let weight = calculator.calculate(&auth, auth_metrics.as_ref(), auth_health);

            // Extract provider from auth ID (format: "provider-key" or "provider-name-key")
            let provider = self.extract_provider(&auth.id);

            weighted_auths.push(WeightedAuth {
                id: auth.id,
                weight,
                provider,
            });
        }

        weighted_auths
    }

    /// Apply provider diversity to favor different providers
    fn apply_provider_diversity(&self, weighted_auths: Vec<WeightedAuth>) -> Vec<WeightedAuth> {
        if !self.config.prefer_diverse_providers {
            return weighted_auths;
        }

        let mut diversified = Vec::new();
        let mut used_providers: HashSet<String> = HashSet::new();
        let mut remaining = Vec::new();

        // First pass: pick highest weighted auth from each provider
        for auth in weighted_auths.into_iter() {
            if let Some(ref provider) = auth.provider {
                if !used_providers.contains(provider) {
                    used_providers.insert(provider.clone());
                    diversified.push(auth);
                } else {
                    remaining.push(auth);
                }
            } else {
                // No provider info, add to remaining
                remaining.push(auth);
            }
        }

        // Second pass: add remaining auths (might duplicate providers)
        diversified.extend(remaining);

        diversified
    }

    /// Ensure the primary auth is first in the list
    fn ensure_primary_first(
        &self,
        mut auths: Vec<WeightedAuth>,
        primary_id: &str,
    ) -> Vec<WeightedAuth> {
        // Find and remove primary
        let primary_pos = auths.iter().position(|w| w.id == primary_id);

        if let Some(pos) = primary_pos {
            let primary = auths.remove(pos);
            auths.insert(0, primary);
        }

        auths
    }

    /// Extract provider from auth ID using known provider patterns.
    ///
    /// Matches against a known provider list (longest prefix first).
    /// Falls back to first-segment heuristic for unrecognized providers.
    fn extract_provider(&self, auth_id: &str) -> Option<String> {
        if auth_id.is_empty() {
            return None;
        }

        let lower = auth_id.to_lowercase();

        // Try known providers (longest first for greedy match)
        for provider in KNOWN_PROVIDERS {
            if let Some(after) = lower.strip_prefix(provider) {
                // Ensure the match ends at a delimiter or end of string
                if after.is_empty()
                    || after.starts_with('-')
                    || after.starts_with('_')
                    || after.starts_with(':')
                {
                    return Some(provider.to_string());
                }
            }
        }

        // Fallback: first segment
        let first = auth_id.split(&['-', '_', ':'][..]).next()?;
        if first.is_empty() {
            None
        } else {
            Some(first.to_string())
        }
    }

    /// Get config
    pub fn config(&self) -> &FallbackConfig {
        &self.config
    }

    /// Set config
    pub fn set_config(&mut self, config: FallbackConfig) {
        self.config = config;
    }
}

impl Default for FallbackPlanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal struct for weighted auth with provider info
#[derive(Debug, Clone)]
struct WeightedAuth {
    id: String,
    weight: f64,
    provider: Option<String>,
}

/// Known provider identifiers, ordered by length (longest first) for greedy matching.
const KNOWN_PROVIDERS: &[&str] = &[
    "azure-openai",
    "amazon-bedrock",
    "byte-dance",
    "google-deepmind",
    "alibaba-cloud",
    "deepseek",
    "openai",
    "google",
    "chrome",
    "xai",
    "mistral",
    "cohere",
    "perplexity",
    "zhipu",
    "baidu",
    "moonshot",
    "meta",
    "azure",
    "bedrock",
    "alibaba",
    "qwen",
    "kimi",
    "grok",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WeightConfig;
    use crate::weight::DefaultWeightCalculator;

    fn create_test_auth(id: &str, provider: Option<&str>) -> AuthInfo {
        // If provider is specified, format the ID as "provider-key"
        let id = if let Some(p) = provider {
            format!("{}-{}", p, id)
        } else {
            id.to_string()
        };

        AuthInfo {
            id,
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        }
    }

    #[tokio::test]
    async fn test_generate_fallbacks_basic() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("auth1", Some("anthropic")),
            create_test_auth("auth2", Some("openai")),
            create_test_auth("auth3", Some("anthropic")),
        ];

        // Initialize metrics
        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        assert!(!fallbacks.is_empty());
        assert_eq!(fallbacks[0].position, 0);
    }

    #[tokio::test]
    async fn test_generate_fallbacks_with_primary() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("auth1", Some("anthropic")),
            create_test_auth("auth2", Some("openai")),
            create_test_auth("auth3", Some("google")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let primary_id = auths[1].id.clone();
        let fallbacks = planner
            .generate_fallbacks(
                auths,
                Some(primary_id.clone()),
                &calculator,
                &metrics,
                &health,
            )
            .await;

        assert!(!fallbacks.is_empty());
        assert_eq!(fallbacks[0].auth_id, primary_id);
        assert_eq!(fallbacks[0].position, 0);
    }

    #[tokio::test]
    async fn test_provider_diversity() {
        let config = FallbackConfig {
            max_fallbacks: 10,
            min_fallbacks: 2,
            enable_provider_diversity: true,
            prefer_diverse_providers: true,
        };
        let planner = FallbackPlanner::with_config(config);
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("key1", Some("anthropic")),
            create_test_auth("key2", Some("anthropic")),
            create_test_auth("key3", Some("openai")),
            create_test_auth("key4", Some("google")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        // First two should be from different providers (highest weight from each)
        let providers: Vec<_> = fallbacks
            .iter()
            .filter_map(|f| f.provider.as_ref())
            .collect();

        if providers.len() >= 2 {
            // Check that first two are from different providers if possible
            assert_ne!(providers[0], providers[1]);
        }
    }

    #[tokio::test]
    async fn test_max_fallbacks_limit() {
        let config = FallbackConfig {
            max_fallbacks: 2,
            min_fallbacks: 1,
            enable_provider_diversity: false,
            prefer_diverse_providers: false,
        };
        let planner = FallbackPlanner::with_config(config);
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("auth1", Some("anthropic")),
            create_test_auth("auth2", Some("openai")),
            create_test_auth("auth3", Some("google")),
            create_test_auth("auth4", Some("cohere")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        assert!(fallbacks.len() <= 2);
    }

    #[tokio::test]
    async fn test_empty_auths_returns_empty() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths: Vec<AuthInfo> = vec![];

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        assert!(fallbacks.is_empty());
    }

    #[tokio::test]
    async fn test_unavailable_auths_filtered() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let mut auth1 = create_test_auth("auth1", Some("anthropic"));
        auth1.unavailable = true;

        let auth2 = create_test_auth("auth2", Some("openai"));

        let auths = vec![auth1, auth2];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        // Only auth2 should be in fallbacks (auth1 is unavailable)
        assert_eq!(fallbacks.len(), 1);
        assert!(fallbacks[0].auth_id.contains("openai"));
    }

    #[tokio::test]
    async fn test_fallback_ordering_by_weight() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("low_priority", Some("anthropic")),
            create_test_auth("high_priority", Some("openai")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        // Fallbacks should be ordered
        for (i, f) in fallbacks.iter().enumerate() {
            assert_eq!(f.position, i);
        }

        // Weights should be non-increasing
        for window in fallbacks.windows(2) {
            assert!(
                window[0].weight >= window[1].weight,
                "Weights should be non-increasing: {} >= {}",
                window[0].weight,
                window[1].weight
            );
        }
    }

    #[tokio::test]
    async fn test_extract_provider() {
        let planner = FallbackPlanner::new();

        // Test various formats
        assert_eq!(
            planner.extract_provider("anthropic-key"),
            Some("anthropic".to_string())
        );
        assert_eq!(
            planner.extract_provider("openai-key-123"),
            Some("openai".to_string())
        );
        assert_eq!(
            planner.extract_provider("google_model_key"),
            Some("google".to_string())
        );
        assert_eq!(
            planner.extract_provider("cohere:key"),
            Some("cohere".to_string())
        );
        assert_eq!(planner.extract_provider(""), None);
    }

    #[test]
    fn test_extract_provider_multi_word_providers() {
        let planner = FallbackPlanner::new();

        // amazon-bedrock should be recognized as a single provider, not "amazon"
        assert_eq!(
            planner.extract_provider("amazon-bedrock-us-east-1-key"),
            Some("amazon-bedrock".to_string())
        );
        assert_eq!(
            planner.extract_provider("azure-openai-gpt4-key"),
            Some("azure-openai".to_string())
        );
    }

    #[test]
    fn test_extract_provider_nested_paths() {
        let planner = FallbackPlanner::new();

        // Standard single-segment providers still work
        assert_eq!(
            planner.extract_provider("deepseek-key"),
            Some("deepseek".to_string())
        );
        assert_eq!(
            planner.extract_provider("xai-grok-key"),
            Some("xai".to_string())
        );
    }

    #[test]
    fn test_extract_provider_unknown_prefix_falls_back() {
        let planner = FallbackPlanner::new();

        // Unknown provider falls back to first segment
        assert_eq!(
            planner.extract_provider("my-custom-provider-key"),
            Some("my".to_string())
        );
        assert_eq!(
            planner.extract_provider("single"),
            Some("single".to_string())
        );
    }
    #[tokio::test]
    async fn test_limited_candidates_min_fallbacks() {
        let config = FallbackConfig {
            max_fallbacks: 5,
            min_fallbacks: 2,
            enable_provider_diversity: false,
            prefer_diverse_providers: false,
        };
        let planner = FallbackPlanner::with_config(config);
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        // Only 1 available auth
        let auths = vec![create_test_auth("auth1", Some("anthropic"))];

        metrics.initialize_auth(&auths[0].id).await;

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        // Should return what's available (1), even though min_fallbacks is 2
        assert_eq!(fallbacks.len(), 1);
    }

    #[tokio::test]
    async fn test_different_auth_credentials_for_fallbacks() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("key1", Some("anthropic")),
            create_test_auth("key2", Some("anthropic")),
            create_test_auth("key3", Some("openai")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        // All fallbacks should have different auth IDs
        let auth_ids: HashSet<_> = fallbacks.iter().map(|f| f.auth_id.clone()).collect();
        assert_eq!(auth_ids.len(), fallbacks.len());
    }

    // ============================================================
    // Edge Case Tests for FallbackPlanner
    // ============================================================

    #[tokio::test]
    async fn test_min_fallbacks_greater_than_available_auths() {
        let config = FallbackConfig {
            max_fallbacks: 10,
            min_fallbacks: 5, // More than available
            enable_provider_diversity: false,
            prefer_diverse_providers: false,
        };
        let planner = FallbackPlanner::with_config(config);
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        // Only 2 available auths
        let auths = vec![
            create_test_auth("auth1", Some("anthropic")),
            create_test_auth("auth2", Some("openai")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        // Should return what's available (2), not min_fallbacks (5)
        assert_eq!(
            fallbacks.len(),
            2,
            "Should return available auths when min_fallbacks > available"
        );
    }

    #[tokio::test]
    async fn test_max_fallbacks_zero_returns_empty() {
        let config = FallbackConfig {
            max_fallbacks: 0,
            min_fallbacks: 0,
            enable_provider_diversity: false,
            prefer_diverse_providers: false,
        };
        let planner = FallbackPlanner::with_config(config);
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("auth1", Some("anthropic")),
            create_test_auth("auth2", Some("openai")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        assert!(
            fallbacks.is_empty(),
            "max_fallbacks=0 should return empty list"
        );
    }

    #[tokio::test]
    async fn test_provider_diversity_all_same_provider() {
        let config = FallbackConfig {
            max_fallbacks: 5,
            min_fallbacks: 2,
            enable_provider_diversity: true,
            prefer_diverse_providers: true,
        };
        let planner = FallbackPlanner::with_config(config);
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        // All auths from same provider
        let auths = vec![
            create_test_auth("key1", Some("anthropic")),
            create_test_auth("key2", Some("anthropic")),
            create_test_auth("key3", Some("anthropic")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        // Should still return fallbacks, just all from same provider
        assert_eq!(
            fallbacks.len(),
            3,
            "Should return all auths even with same provider"
        );

        // All should be from anthropic
        for f in &fallbacks {
            assert_eq!(
                f.provider,
                Some("anthropic".to_string()),
                "All should be from anthropic"
            );
        }
    }

    #[tokio::test]
    async fn test_primary_auth_not_in_available_list() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("auth1", Some("anthropic")),
            create_test_auth("auth2", Some("openai")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        // Primary that doesn't exist in available auths
        let fallbacks = planner
            .generate_fallbacks(
                auths,
                Some("non-existent-primary".to_string()),
                &calculator,
                &metrics,
                &health,
            )
            .await;

        // Should still return available auths, just won't have primary first
        assert_eq!(fallbacks.len(), 2);
        // First won't be the non-existent primary
        assert_ne!(fallbacks[0].auth_id, "non-existent-primary");
    }

    #[tokio::test]
    async fn test_fallback_ordering_weight_descending() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("low", Some("anthropic")),
            create_test_auth("high", Some("openai")),
            create_test_auth("mid", Some("google")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        // Weights should be non-increasing (descending order)
        for window in fallbacks.windows(2) {
            assert!(
                window[0].weight >= window[1].weight,
                "Weights should be in descending order: {} >= {}",
                window[0].weight,
                window[1].weight
            );
        }
    }

    #[tokio::test]
    async fn test_fallback_with_all_unavailable_auths() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        // All auths marked unavailable
        let mut auth1 = create_test_auth("auth1", Some("anthropic"));
        auth1.unavailable = true;
        let mut auth2 = create_test_auth("auth2", Some("openai"));
        auth2.unavailable = true;

        let auths = vec![auth1, auth2];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        assert!(
            fallbacks.is_empty(),
            "All unavailable auths should result in empty fallbacks"
        );
    }
}
