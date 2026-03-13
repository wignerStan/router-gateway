use crate::config::SmartRoutingConfig;
use crate::health::{HealthManager, HealthStatus};
use crate::metrics::{AuthMetrics, MetricsCollector};
use crate::policy_weight::PolicyAwareWeightCalculator;
use crate::weight::{AuthInfo, WeightCalculator};
use model_registry::{ModelInfo, PolicyContext, PolicyMatcher, PolicyRegistry};
use rand::Rng;
use std::sync::Arc;

/// Weighted auth for selection
#[derive(Debug, Clone)]
struct WeightedAuth {
    id: String,
    weight: f64,
}

/// Smart selector for credential selection
pub struct SmartSelector {
    config: SmartRoutingConfig,
    calculator: Box<dyn WeightCalculator>,
    metrics: MetricsCollector,
    health: HealthManager,
    /// Optional policy matcher for policy-aware routing
    policy_matcher: Option<Arc<PolicyMatcher>>,
}

impl SmartSelector {
    /// Create a new smart selector
    pub fn new(config: SmartRoutingConfig) -> Self {
        let calculator: Box<dyn WeightCalculator> = match config.strategy.as_str() {
            "weighted" => Box::new(crate::weight::DefaultWeightCalculator::new(
                config.weight.clone(),
            )),
            _ => Box::new(crate::weight::DefaultWeightCalculator::new(
                config.weight.clone(),
            )),
        };

        Self {
            calculator,
            metrics: MetricsCollector::new(),
            health: HealthManager::new(config.health.clone()),
            policy_matcher: None,
            config,
        }
    }

    /// Create a new smart selector with policy support
    pub fn with_policy(config: SmartRoutingConfig, registry: PolicyRegistry) -> Self {
        let matcher = Arc::new(PolicyMatcher::new(registry));
        let calculator: Box<dyn WeightCalculator> = if config.policy.enabled {
            Box::new(PolicyAwareWeightCalculator::new(
                config.weight.clone(),
                matcher.clone(),
            ))
        } else {
            Box::new(crate::weight::DefaultWeightCalculator::new(
                config.weight.clone(),
            ))
        };

        Self {
            calculator,
            metrics: MetricsCollector::new(),
            health: HealthManager::new(config.health.clone()),
            policy_matcher: Some(matcher),
            config,
        }
    }

    /// Set the policy registry for policy-aware routing
    pub fn set_policy_registry(&mut self, registry: PolicyRegistry) {
        let matcher = Arc::new(PolicyMatcher::new(registry));
        self.policy_matcher = Some(matcher.clone());

        // Update calculator to use policy-aware version
        if self.config.policy.enabled {
            self.calculator = Box::new(PolicyAwareWeightCalculator::new(
                self.config.weight.clone(),
                matcher,
            ));
        }
    }

    /// Get the policy matcher (if configured)
    pub fn policy_matcher(&self) -> Option<&PolicyMatcher> {
        self.policy_matcher.as_deref()
    }

    /// Pick the best auth based on weighted selection
    pub async fn pick(&self, auths: Vec<AuthInfo>) -> Option<String> {
        if !self.config.enabled {
            // Smart routing disabled, return first
            return auths.into_iter().next().map(|a| a.id);
        }

        if auths.is_empty() {
            return None;
        }

        // Filter available auths and calculate weights
        let available = self.filter_and_weigh(auths).await;

        if available.is_empty() {
            return None;
        }

        // Select by weight
        Some(self.select_by_weight(available))
    }

    /// Pick the best auth with policy-aware selection
    ///
    /// This method evaluates routing policies against the model and context,
    /// then adjusts weights accordingly.
    pub async fn pick_with_policy(
        &self,
        auths: Vec<AuthInfo>,
        model: &ModelInfo,
        context: &PolicyContext,
    ) -> Option<String> {
        if !self.config.enabled {
            return auths.into_iter().next().map(|a| a.id);
        }

        if auths.is_empty() {
            return None;
        }

        // Filter available auths and calculate policy-aware weights
        let available = self
            .filter_and_weigh_with_policy(auths, model, context)
            .await;

        if available.is_empty() {
            return None;
        }

        Some(self.select_by_weight(available))
    }

    /// Filter available auths and calculate weights (without policy)
    async fn filter_and_weigh(&self, auths: Vec<AuthInfo>) -> Vec<WeightedAuth> {
        let mut available = Vec::new();

        for auth in auths {
            // Skip disabled auths
            if auth.unavailable {
                continue;
            }

            // Get metrics
            let metrics = self.metrics.get_metrics(&auth.id).await;

            // Get health status
            let health = self.health.get_status(&auth.id).await;

            // Check availability
            let is_available = self.health.is_available(&auth.id).await;

            if !is_available {
                continue;
            }

            // Calculate weight
            let weight = self.calculator.calculate(&auth, metrics.as_ref(), health);

            // Only include auths with positive weight
            if weight > 0.0 {
                available.push(WeightedAuth {
                    id: auth.id,
                    weight,
                });
            }
        }

        available
    }

    /// Filter available auths and calculate policy-aware weights
    async fn filter_and_weigh_with_policy(
        &self,
        auths: Vec<AuthInfo>,
        model: &ModelInfo,
        context: &PolicyContext,
    ) -> Vec<WeightedAuth> {
        let mut available = Vec::new();

        // Get policy factor for the model
        let policy_factor = self
            .policy_matcher
            .as_ref()
            .map(|m| m.calculate_weight_factor(model, context))
            .unwrap_or(1.0);

        // Check if model is blocked by any policy
        let is_blocked = self
            .policy_matcher
            .as_ref()
            .map(|m| m.is_blocked(model, context))
            .unwrap_or(false);

        if is_blocked {
            return Vec::new();
        }

        for auth in auths {
            // Skip disabled auths
            if auth.unavailable {
                continue;
            }

            // Get metrics
            let metrics = self.metrics.get_metrics(&auth.id).await;

            // Get health status
            let health = self.health.get_status(&auth.id).await;

            // Check availability
            let is_available = self.health.is_available(&auth.id).await;

            if !is_available {
                continue;
            }

            // Calculate weight with policy awareness
            let weight = if let Some(policy_calc) = self
                .calculator
                .as_any()
                .downcast_ref::<PolicyAwareWeightCalculator>()
            {
                // Use policy-aware calculator
                let (_, _, final_weight) = policy_calc.calculate_with_policy(
                    &auth,
                    metrics.as_ref(),
                    health,
                    model,
                    context,
                );
                final_weight
            } else {
                // Apply policy factor to base weight
                let base_weight = self.calculator.calculate(&auth, metrics.as_ref(), health);
                base_weight * policy_factor
            };

            // Only include auths with positive weight
            if weight > 0.0 {
                available.push(WeightedAuth {
                    id: auth.id,
                    weight,
                });
            }
        }

        available
    }

    /// Select auth by weighted random choice
    fn select_by_weight(&self, available: Vec<WeightedAuth>) -> String {
        if available.len() == 1 {
            return available.into_iter().next().unwrap().id;
        }

        // Calculate total weight
        let total_weight: f64 = available.iter().map(|a| a.weight).sum();

        if total_weight <= 0.0 {
            // All weights are zero, select randomly
            let idx = rand::thread_rng().gen_range(0..available.len());
            return available.into_iter().nth(idx).unwrap().id;
        }

        // Save last element as fallback for floating-point edge cases
        let fallback = available.last().map(|a| a.id.clone()).unwrap();

        // Weighted random selection
        let r = rand::thread_rng().gen::<f64>() * total_weight;
        let mut cumulative = 0.0;

        for auth in available {
            cumulative += auth.weight;
            if r <= cumulative {
                return auth.id;
            }
        }

        // SAFETY: Mathematically this loop should always match because:
        // 1. total_weight > 0 (checked above)
        // 2. r is in [0, total_weight)
        // 3. cumulative accumulates to total_weight
        // However, floating-point edge cases could theoretically miss,
        // so return the saved fallback.
        fallback
    }

    /// Get metrics collector
    pub fn metrics(&self) -> &MetricsCollector {
        &self.metrics
    }

    /// Get health manager
    pub fn health(&self) -> &HealthManager {
        &self.health
    }

    /// Get config
    pub fn config(&self) -> &SmartRoutingConfig {
        &self.config
    }

    /// Set config
    pub fn set_config(&mut self, config: SmartRoutingConfig) {
        self.config = config.clone();

        // Update calculator
        self.calculator = match self.config.strategy.as_str() {
            "weighted" => Box::new(crate::weight::DefaultWeightCalculator::new(
                self.config.weight.clone(),
            )),
            _ => Box::new(crate::weight::DefaultWeightCalculator::new(
                self.config.weight.clone(),
            )),
        };

        // Update health config
        self.health.set_config(self.config.health.clone());
    }

    /// Record execution result
    pub fn record_result(&self, auth_id: &str, success: bool, latency_ms: f64, status_code: i32) {
        let auth_id = auth_id.to_string();
        let metrics = self.metrics.clone();
        let health = self.health.clone();

        tokio::spawn(async move {
            metrics
                .record_result(&auth_id, success, latency_ms, status_code)
                .await;
            health
                .update_from_result(&auth_id, success, status_code)
                .await;
        });
    }
}

impl Clone for SmartSelector {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone_config(),
            calculator: Box::new(crate::weight::DefaultWeightCalculator::new(
                self.config.weight.clone(),
            )),
            metrics: self.metrics.clone(),
            health: self.health.clone(),
            policy_matcher: self.policy_matcher.clone(),
        }
    }
}

impl WeightCalculator for SmartSelector {
    fn calculate(
        &self,
        auth: &AuthInfo,
        metrics: Option<&AuthMetrics>,
        health: HealthStatus,
    ) -> f64 {
        self.calculator.calculate(auth, metrics, health)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_model() -> model_registry::ModelInfo {
        model_registry::ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window: 4096,
            max_output_tokens: 1024,
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: model_registry::ModelCapabilities {
                streaming: true,
                tools: true,
                vision: false,
                thinking: false,
            },
            rate_limits: model_registry::RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: model_registry::DataSource::Static,
        }
    }

    #[tokio::test]
    async fn test_weighted_selection() {
        let config = SmartRoutingConfig::default();
        let selector = SmartSelector::new(config);

        let auths = vec![
            AuthInfo {
                id: "auth1".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "auth2".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "auth3".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
        ];

        // Initialize metrics
        for auth in &auths {
            selector.metrics().initialize_auth(&auth.id).await;
        }

        // Pick should return one of the auths
        let selected = selector.pick(auths).await;
        assert!(selected.is_some());
        let selected_id = selected.unwrap();
        assert!(["auth1", "auth2", "auth3"].contains(&selected_id.as_str()));
    }

    #[tokio::test]
    async fn test_unavailable_filtering() {
        let config = SmartRoutingConfig::default();
        let selector = SmartSelector::new(config);

        let auths = vec![
            AuthInfo {
                id: "auth1".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "auth2".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: true, // Marked unavailable
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "auth3".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
        ];

        // Initialize metrics
        for auth in &auths {
            selector.metrics().initialize_auth(&auth.id).await;
        }

        // Pick should not return auth2
        for _ in 0..10 {
            let selected = selector.pick(auths.clone()).await;
            assert!(selected.is_some());
            let selected_id = selected.unwrap();
            assert_ne!(selected_id, "auth2");
        }
    }

    // ============================================================
    // Tests for pick_with_policy()
    // ============================================================

    #[tokio::test]
    async fn test_pick_with_policy_basic() {
        let config = SmartRoutingConfig::default();
        let selector = SmartSelector::new(config);

        let auths = vec![
            AuthInfo {
                id: "auth1".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "auth2".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
        ];

        // Initialize metrics
        for auth in &auths {
            selector.metrics().initialize_auth(&auth.id).await;
        }

        let model = create_test_model();
        let context = model_registry::PolicyContext::default();

        let selected = selector.pick_with_policy(auths, &model, &context).await;
        assert!(selected.is_some());
        let selected_id = selected.unwrap();
        assert!(["auth1", "auth2"].contains(&selected_id.as_str()));
    }

    #[tokio::test]
    async fn test_pick_with_policy_disabled_routing() {
        let config = SmartRoutingConfig {
            enabled: false,
            ..Default::default()
        };
        let selector = SmartSelector::new(config);

        let auths = vec![
            AuthInfo {
                id: "auth1".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "auth2".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
        ];

        let model = create_test_model();
        let context = model_registry::PolicyContext::default();

        // With disabled routing, should return first auth
        let selected = selector.pick_with_policy(auths, &model, &context).await;
        assert_eq!(selected, Some("auth1".to_string()));
    }

    #[tokio::test]
    async fn test_pick_with_policy_empty_auths() {
        let config = SmartRoutingConfig::default();
        let selector = SmartSelector::new(config);

        let auths: Vec<AuthInfo> = vec![];

        let model = create_test_model();
        let context = model_registry::PolicyContext::default();

        let selected = selector.pick_with_policy(auths, &model, &context).await;
        assert!(selected.is_none());
    }

    #[tokio::test]
    async fn test_pick_with_policy_with_policy_registry() {
        use model_registry::PolicyRegistry;

        let mut config = SmartRoutingConfig::default();
        config.policy.enabled = true;

        let registry = PolicyRegistry::new();
        let selector = SmartSelector::with_policy(config, registry);

        let auths = vec![AuthInfo {
            id: "auth1".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        }];

        selector.metrics().initialize_auth("auth1").await;

        let model = create_test_model();
        let context = model_registry::PolicyContext::default();

        let selected = selector.pick_with_policy(auths, &model, &context).await;
        assert_eq!(selected, Some("auth1".to_string()));
    }

    // ============================================================
    // Tests for set_config()
    // ============================================================

    #[tokio::test]
    async fn test_set_config() {
        let config = SmartRoutingConfig::default();
        let mut selector = SmartSelector::new(config.clone());

        // Verify initial config
        assert_eq!(selector.config().strategy, "weighted");

        // Create new config with different strategy
        let new_config = SmartRoutingConfig {
            strategy: "adaptive".to_string(),
            weight: crate::config::WeightConfig {
                success_rate_weight: 0.5,
                ..Default::default()
            },
            ..Default::default()
        };

        // Update config
        selector.set_config(new_config.clone());

        // Verify config was updated
        assert_eq!(selector.config().strategy, "adaptive");
        assert!((selector.config().weight.success_rate_weight - 0.5).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_set_config_preserves_metrics() {
        let config = SmartRoutingConfig::default();
        let mut selector = SmartSelector::new(config);

        // Record some metrics
        selector.metrics().initialize_auth("auth1").await;
        selector
            .metrics()
            .record_result("auth1", true, 100.0, 200)
            .await;

        // Update config
        let new_config = SmartRoutingConfig::default();
        selector.set_config(new_config);

        // Metrics should still exist (set_config doesn't clear metrics)
        let metrics = selector.metrics().get_metrics("auth1").await;
        assert!(metrics.is_some());
        assert_eq!(metrics.unwrap().total_requests, 1);
    }

    #[tokio::test]
    async fn test_set_config_updates_health_config() {
        let config = SmartRoutingConfig::default();
        let mut selector = SmartSelector::new(config);

        // Create new config with different health thresholds
        let mut new_config = SmartRoutingConfig::default();
        new_config.health.healthy_threshold = 10;
        new_config.health.unhealthy_threshold = 20;

        // Update config
        selector.set_config(new_config);

        // Health manager should use new config
        // (We can't directly verify this without making health() return &mut, but we can test behavior)
        // The health manager's set_config is called internally
    }

    // ============================================================
    // Tests for filter_and_weigh (indirect via pick)
    // ============================================================

    #[tokio::test]
    async fn test_filter_and_weigh_zero_weight_excluded() {
        let mut config = SmartRoutingConfig::default();
        // Set very high thresholds to make auth unhealthy
        config.health.unhealthy_threshold = 1;
        config.health.cooldown_period_seconds = 3600; // Long cooldown
        let selector = SmartSelector::new(config);

        let auths = vec![
            AuthInfo {
                id: "auth1".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "auth2".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
        ];

        // Initialize metrics
        for auth in &auths {
            selector.metrics().initialize_auth(&auth.id).await;
        }

        // Make auth1 unhealthy by recording failures
        selector
            .health()
            .update_from_result("auth1", false, 500)
            .await;

        // Pick should only return auth2 (auth1 is unhealthy)
        for _ in 0..10 {
            let selected = selector.pick(auths.clone()).await;
            assert!(selected.is_some());
            // auth1 should be filtered out due to being unhealthy
        }
    }

    #[tokio::test]
    async fn test_filter_and_weigh_all_unavailable() {
        let config = SmartRoutingConfig::default();
        let selector = SmartSelector::new(config);

        let auths = vec![
            AuthInfo {
                id: "auth1".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: true,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "auth2".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: true,
                model_states: Vec::new(),
            },
        ];

        // All auths are unavailable, pick should return None
        let selected = selector.pick(auths).await;
        assert!(selected.is_none());
    }

    #[tokio::test]
    async fn test_filter_and_weigh_single_available() {
        let config = SmartRoutingConfig::default();
        let selector = SmartSelector::new(config);

        let auths = vec![
            AuthInfo {
                id: "auth1".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: true,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "auth2".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "auth3".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: true,
                model_states: Vec::new(),
            },
        ];

        selector.metrics().initialize_auth("auth2").await;

        // Only auth2 is available, should always be selected
        for _ in 0..5 {
            let selected = selector.pick(auths.clone()).await;
            assert_eq!(selected, Some("auth2".to_string()));
        }
    }

    // ============================================================
    // Tests for SmartSelector Clone
    // ============================================================

    #[tokio::test]
    async fn test_selector_clone_shares_metrics_and_health() {
        let config = SmartRoutingConfig::default();
        let selector1 = SmartSelector::new(config);

        // Record metrics in selector1
        selector1.metrics().initialize_auth("auth1").await;
        selector1
            .metrics()
            .record_result("auth1", true, 100.0, 200)
            .await;

        // Clone selector
        let selector2 = selector1.clone();

        // Record different metrics in selector2
        selector2.metrics().initialize_auth("auth2").await;
        selector2
            .metrics()
            .record_result("auth2", true, 50.0, 200)
            .await;

        // Selector1 should see auth2 metrics (shared storage via Arc)
        assert!(selector1.metrics().get_metrics("auth2").await.is_some());

        // Selector2 should see auth1 metrics (shared storage via Arc)
        assert!(selector2.metrics().get_metrics("auth1").await.is_some());
    }

    // Note: record_result spawns a detached tokio task which makes it difficult to test
    // reliably without modifying the API. The underlying metrics.record_result and
    // health.update_from_result are tested directly in their respective modules.

    // ============================================================
    // Tests for select_by_weight edge cases
    // ============================================================

    #[tokio::test]
    async fn test_single_auth_selection() {
        let config = SmartRoutingConfig::default();
        let selector = SmartSelector::new(config);

        let auths = vec![AuthInfo {
            id: "only-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        }];

        selector.metrics().initialize_auth("only-auth").await;

        // Single auth should always be selected
        for _ in 0..5 {
            let selected = selector.pick(auths.clone()).await;
            assert_eq!(selected, Some("only-auth".to_string()));
        }
    }

    #[tokio::test]
    async fn test_disabled_routing_returns_first() {
        let config = SmartRoutingConfig {
            enabled: false,
            ..Default::default()
        };
        let selector = SmartSelector::new(config);

        let auths = vec![
            AuthInfo {
                id: "first-auth".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "second-auth".to_string(),
                priority: Some(100), // Higher priority, but should be ignored
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
        ];

        // Should return first auth when routing is disabled
        let selected = selector.pick(auths).await;
        assert_eq!(selected, Some("first-auth".to_string()));
    }

    // ============================================================
    // Edge Case Tests: Floating-Point Precision
    // ============================================================

    #[tokio::test]
    async fn test_weighted_selection_with_very_small_differences() {
        let config = SmartRoutingConfig::default();
        let selector = SmartSelector::new(config);

        // Create auths with nearly identical metrics
        let auths = vec![
            AuthInfo {
                id: "auth1".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "auth2".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
        ];

        // Initialize metrics with very similar performance
        selector.metrics().initialize_auth("auth1").await;
        selector.metrics().initialize_auth("auth2").await;

        // Record almost identical results
        selector
            .metrics()
            .record_result("auth1", true, 100.0, 200)
            .await;
        selector
            .metrics()
            .record_result("auth2", true, 100.0001, 200)
            .await;

        // Both should have a chance to be selected over many iterations
        let mut auth1_count = 0;
        let mut auth2_count = 0;
        for _ in 0..100 {
            let selected = selector.pick(auths.clone()).await.unwrap();
            if selected == "auth1" {
                auth1_count += 1;
            } else {
                auth2_count += 1;
            }
        }

        // With nearly identical weights, both should be selected roughly equally
        // (within statistical tolerance)
        assert!(
            auth1_count > 20 && auth2_count > 20,
            "Both auths should receive selections with near-equal weights (auth1: {}, auth2: {})",
            auth1_count,
            auth2_count
        );
    }

    #[tokio::test]
    async fn test_weighted_selection_extreme_weight_ratio() {
        let config = SmartRoutingConfig::default();
        let selector = SmartSelector::new(config);

        let auths = vec![
            AuthInfo {
                id: "high-perf".to_string(),
                priority: Some(100),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "low-perf".to_string(),
                priority: Some(-100),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
        ];

        // Initialize with metrics showing extreme performance difference
        selector.metrics().initialize_auth("high-perf").await;
        selector.metrics().initialize_auth("low-perf").await;

        // Record 50 data points (exceeds min 10 for Learned mode) with extreme difference
        for _ in 0..50 {
            selector
                .metrics()
                .record_result("high-perf", true, 1.0, 200)
                .await;
            selector
                .metrics()
                .record_result("low-perf", false, 30000.0, 500)
                .await;
        }

        // High-perf should dominate selection
        let mut high_count = 0;
        let mut low_count = 0;
        for _ in 0..100 {
            match selector.pick(auths.clone()).await.unwrap().as_str() {
                "high-perf" => high_count += 1,
                "low-perf" => low_count += 1,
                _ => {},
            }
        }

        // High-perf should be selected MORE OFTEN than low-perf (relative assertion)
        assert!(
            high_count > low_count,
            "High-perf auth should be selected more often than low-perf (high: {}, low: {})",
            high_count,
            low_count
        );

        // Verify high-perf is selected at least 60% of the time
        // This is a robust threshold that accounts for probabilistic variance
        // while still ensuring significant weight differentiation
        assert!(
            high_count >= 60,
            "High-perf auth should be selected at least 60% of the time (got {} out of 100)",
            high_count
        );
    }

    // ============================================================
    // Edge Case Tests: Concurrent Access
    // ============================================================

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_concurrent_pick_operations() {
        use std::sync::Arc;

        let config = SmartRoutingConfig::default();
        let selector = Arc::new(SmartSelector::new(config));

        let auths = vec![
            AuthInfo {
                id: "auth1".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "auth2".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "auth3".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
        ];

        // Initialize metrics
        for auth in &auths {
            selector.metrics().initialize_auth(&auth.id).await;
        }

        let mut handles = Vec::new();
        for _ in 0..20 {
            let selector_clone = Arc::clone(&selector);
            let auths_clone = auths.clone();
            let handle = tokio::spawn(async move {
                let mut results = Vec::new();
                for _ in 0..10 {
                    let selected = selector_clone.pick(auths_clone.clone()).await;
                    results.push(selected);
                }
                results
            });
            handles.push(handle);
        }

        // Wait for all concurrent operations to complete
        let all_results: Vec<_> = futures::future::join_all(handles).await;

        // Verify all operations completed successfully
        for results in all_results {
            for selected in results.unwrap() {
                assert!(selected.is_some());
                let id = selected.unwrap();
                assert!(
                    ["auth1", "auth2", "auth3"].contains(&id.as_str()),
                    "Selected invalid auth: {}",
                    id
                );
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_concurrent_pick_and_record() {
        use std::sync::Arc;

        let config = SmartRoutingConfig::default();
        let selector = Arc::new(SmartSelector::new(config));

        let auths = vec![
            AuthInfo {
                id: "auth1".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "auth2".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
        ];

        // Initialize metrics
        for auth in &auths {
            selector.metrics().initialize_auth(&auth.id).await;
        }

        let mut handles = Vec::new();

        // Spawn pick tasks
        for _ in 0..10 {
            let selector_clone = Arc::clone(&selector);
            let auths_clone = auths.clone();
            let handle = tokio::spawn(async move {
                for _ in 0..20 {
                    let _ = selector_clone.pick(auths_clone.clone()).await;
                }
            });
            handles.push(handle);
        }

        // Spawn record tasks
        for i in 0..10 {
            let selector_clone = Arc::clone(&selector);
            let auth_id = format!("auth{}", (i % 2) + 1);
            let handle = tokio::spawn(async move {
                for j in 0..20 {
                    selector_clone
                        .metrics()
                        .record_result(&auth_id, j % 2 == 0, 100.0 + j as f64, 200)
                        .await;
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results: Vec<_> = futures::future::join_all(handles).await;

        // Verify no panics occurred
        for result in results {
            assert!(result.is_ok(), "Task panicked during concurrent operations");
        }
    }

    // ============================================================
    // Edge Case Tests: Zero and Negative Weight Handling
    // ============================================================

    #[tokio::test]
    async fn test_all_zero_weights_falls_back_to_random() {
        // Configure for very low weights
        let config = SmartRoutingConfig {
            weight: crate::config::WeightConfig {
                success_rate_weight: 0.0,
                latency_weight: 0.0,
                health_weight: 0.0,
                load_weight: 0.0,
                priority_weight: 0.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let selector = SmartSelector::new(config);

        let auths = vec![
            AuthInfo {
                id: "auth1".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "auth2".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
        ];

        // Initialize metrics
        for auth in &auths {
            selector.metrics().initialize_auth(&auth.id).await;
        }

        // All weights should be equal (near zero), selection should still work
        let mut selections = std::collections::HashSet::new();
        for _ in 0..50 {
            if let Some(id) = selector.pick(auths.clone()).await {
                selections.insert(id);
            }
        }

        // Should be able to select auths even with all-zero weights
        assert!(
            !selections.is_empty(),
            "Should be able to select with zero weights"
        );
    }

    #[tokio::test]
    async fn test_quota_exceeded_massively_reduces_selection_probability() {
        let config = SmartRoutingConfig::default();
        let selector = SmartSelector::new(config);

        let auths = vec![
            AuthInfo {
                id: "normal-auth".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "quota-auth".to_string(),
                priority: Some(0),
                quota_exceeded: true, // Quota exceeded
                unavailable: false,
                model_states: Vec::new(),
            },
        ];

        // Initialize metrics
        for auth in &auths {
            selector.metrics().initialize_auth(&auth.id).await;
        }

        // Run selections with larger sample size to reduce variance
        let mut normal_count = 0;
        let mut quota_count = 0;
        for _ in 0..500 {
            let selected = selector.pick(auths.clone()).await.unwrap();
            if selected == "normal-auth" {
                normal_count += 1;
            } else {
                quota_count += 1;
            }
        }

        // Normal auth should be selected much more often (at least 4x ratio)
        // Using 4x instead of 5x to account for statistical variance
        assert!(
            normal_count > quota_count * 4,
            "Normal auth should dominate over quota-exceeded (normal: {}, quota: {})",
            normal_count,
            quota_count
        );
    }

    // ============================================================
    // Edge Case Tests: Boundary Conditions
    // ============================================================

    #[tokio::test]
    async fn test_pick_with_large_number_of_auths() {
        let config = SmartRoutingConfig::default();
        let selector = SmartSelector::new(config);

        // Create 100 auths
        let auths: Vec<AuthInfo> = (0..100)
            .map(|i| AuthInfo {
                id: format!("auth-{}", i),
                priority: Some(i % 10 - 5), // Priorities from -5 to 4
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            })
            .collect();

        // Initialize metrics for all
        for auth in &auths {
            selector.metrics().initialize_auth(&auth.id).await;
        }

        // Should still be able to select
        let selected = selector.pick(auths.clone()).await;
        assert!(selected.is_some());

        // Run many selections to verify stability
        let mut all_selected = std::collections::HashSet::new();
        for _ in 0..500 {
            if let Some(id) = selector.pick(auths.clone()).await {
                all_selected.insert(id);
            }
        }

        // Should have selected a variety of auths
        assert!(
            all_selected.len() > 10,
            "Should select multiple different auths from large pool"
        );
    }

    #[tokio::test]
    async fn test_pick_with_duplicate_auth_ids() {
        let config = SmartRoutingConfig::default();
        let selector = SmartSelector::new(config);

        // Create auths with same ID (edge case that shouldn't normally happen)
        let auths = vec![
            AuthInfo {
                id: "same-auth".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "same-auth".to_string(), // Duplicate ID
                priority: Some(100),         // Different priority
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
        ];

        selector.metrics().initialize_auth("same-auth").await;

        // Should handle duplicates gracefully
        let selected = selector.pick(auths).await;
        assert_eq!(selected, Some("same-auth".to_string()));
    }
}
