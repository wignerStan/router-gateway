#![allow(clippy::unreadable_literal)]
//! Policy-aware weight calculator for multi-dimensional routing
//!
//! This module extends the default weight calculator to incorporate
//! policy-based routing decisions from the model-registry policy system.

use std::sync::Arc;

use crate::config::WeightConfig;
use crate::health::HealthStatus;
use crate::metrics::AuthMetrics;
use crate::weight::{AuthInfo, DefaultWeightCalculator, WeightCalculator};
use model_registry::{ModelInfo, PolicyContext, PolicyMatcher};

/// Policy-aware weight calculator that combines base weights with policy scores
///
/// This calculator wraps a default weight calculator and multiplies the base
/// weight by a policy factor derived from matching routing policies.
pub struct PolicyAwareWeightCalculator {
    /// Base weight calculator for standard metrics
    base_calculator: DefaultWeightCalculator,
    /// Policy matcher for evaluating routing policies
    matcher: Arc<PolicyMatcher>,
    /// Weight configuration
    config: WeightConfig,
}

impl PolicyAwareWeightCalculator {
    /// Create a new policy-aware weight calculator
    #[must_use]
    pub fn new(config: WeightConfig, matcher: Arc<PolicyMatcher>) -> Self {
        let base_calculator = DefaultWeightCalculator::new(config.clone());
        Self {
            base_calculator,
            matcher,
            config,
        }
    }

    /// Calculate the policy weight factor for a model in the given context
    ///
    /// This method evaluates all policies against the model and combines
    /// their scores into a single multiplicative factor.
    #[must_use]
    pub fn calculate_policy_factor(&self, model: &ModelInfo, context: &PolicyContext) -> f64 {
        // Check if model is blocked by any policy
        if self.matcher.is_blocked(model, context) {
            return 0.0;
        }

        // Get combined policy weight factor
        self.matcher.calculate_weight_factor(model, context)
    }

    /// Calculate full weight including policy factor
    ///
    /// Returns (`base_weight`, `policy_factor`, `final_weight`)
    #[must_use]
    pub fn calculate_with_policy(
        &self,
        auth: &AuthInfo,
        metrics: Option<&AuthMetrics>,
        health: HealthStatus,
        model: &ModelInfo,
        context: &PolicyContext,
    ) -> (f64, f64, f64) {
        let base_weight = self.base_calculator.calculate(auth, metrics, health);
        let policy_factor = self.calculate_policy_factor(model, context);
        let final_weight = base_weight * policy_factor;

        (base_weight, policy_factor, final_weight)
    }

    /// Get reference to the underlying policy matcher
    #[must_use]
    pub fn matcher(&self) -> &PolicyMatcher {
        &self.matcher
    }

    /// Get reference to the weight configuration
    #[must_use]
    pub const fn config(&self) -> &WeightConfig {
        &self.config
    }
}

impl WeightCalculator for PolicyAwareWeightCalculator {
    fn calculate(
        &self,
        auth: &AuthInfo,
        metrics: Option<&AuthMetrics>,
        health: HealthStatus,
    ) -> f64 {
        // For the trait method, we can only calculate base weight
        // Use calculate_with_policy for full policy-aware calculation
        self.base_calculator.calculate(auth, metrics, health)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Extended weight calculator trait that supports policy-aware calculation
pub trait PolicyWeightCalculator: WeightCalculator {
    /// Calculate weight with policy context
    fn calculate_with_context(
        &self,
        auth: &AuthInfo,
        metrics: Option<&AuthMetrics>,
        health: HealthStatus,
        model: &ModelInfo,
        context: &PolicyContext,
    ) -> f64;
}

impl PolicyWeightCalculator for PolicyAwareWeightCalculator {
    fn calculate_with_context(
        &self,
        auth: &AuthInfo,
        metrics: Option<&AuthMetrics>,
        health: HealthStatus,
        model: &ModelInfo,
        context: &PolicyContext,
    ) -> f64 {
        let (_, _, final_weight) =
            self.calculate_with_policy(auth, metrics, health, model, context);
        final_weight
    }
}

/// Factory for creating weight calculators based on routing strategy
pub struct WeightCalculatorFactory;

impl WeightCalculatorFactory {
    /// Create a weight calculator for the given strategy
    #[must_use]
    pub fn create(
        strategy: &str,
        config: WeightConfig,
        matcher: Option<Arc<PolicyMatcher>>,
    ) -> Box<dyn WeightCalculator> {
        match strategy {
            "policy_aware" => {
                let matcher = matcher.unwrap_or_else(|| Arc::new(PolicyMatcher::empty()));
                Box::new(PolicyAwareWeightCalculator::new(config, matcher))
            },
            _ => Box::new(DefaultWeightCalculator::new(config)),
        }
    }

    /// Create a policy-aware weight calculator
    #[must_use]
    pub fn create_policy_aware(
        config: WeightConfig,
        matcher: Arc<PolicyMatcher>,
    ) -> PolicyAwareWeightCalculator {
        PolicyAwareWeightCalculator::new(config, matcher)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model_registry::{
        templates, DataSource, ModelCapabilities, PolicyRegistry, RateLimits, RoutingPolicy,
    };

    fn create_test_model(id: &str, provider: &str, price: f64) -> ModelInfo {
        ModelInfo {
            id: id.to_string(),
            name: "Test Model".to_string(),
            provider: provider.to_string(),
            context_window: 200000,
            max_output_tokens: 4096,
            input_price_per_million: price,
            output_price_per_million: price * 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: true,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        }
    }

    fn create_test_auth() -> AuthInfo {
        AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        }
    }

    #[test]
    fn test_policy_aware_calculator_basic() {
        let config = WeightConfig::default();
        let matcher = Arc::new(PolicyMatcher::empty());
        let calculator = PolicyAwareWeightCalculator::new(config, matcher);

        let auth = create_test_auth();
        let model = create_test_model("test", "test", 3.0);
        let context = PolicyContext::default();

        // With empty matcher, policy factor should be 1.0
        let policy_factor = calculator.calculate_policy_factor(&model, &context);
        assert!(
            (policy_factor - 1.0).abs() < 0.01,
            "Empty matcher should return factor of 1.0"
        );

        // Full calculation
        let (base, factor, final_weight) =
            calculator.calculate_with_policy(&auth, None, HealthStatus::Healthy, &model, &context);
        assert!(base > 0.0);
        assert!((factor - 1.0).abs() < 0.01);
        assert!((final_weight - base).abs() < 0.01);
    }

    #[test]
    fn test_policy_aware_calculator_with_policy() {
        let mut registry = PolicyRegistry::new();
        registry.add(templates::vision_required().with_priority(10));

        let matcher = Arc::new(PolicyMatcher::new(registry));
        let config = WeightConfig::default();
        let calculator = PolicyAwareWeightCalculator::new(config, matcher);

        let auth = create_test_auth();
        let model = create_test_model("claude-sonnet", "anthropic", 3.0);
        let context = PolicyContext::default();

        let (base, factor, final_weight) =
            calculator.calculate_with_policy(&auth, None, HealthStatus::Healthy, &model, &context);

        // Vision policy should boost the weight
        assert!(base > 0.0);
        assert!(factor > 1.0, "Vision policy should boost factor above 1.0");
        assert!(
            final_weight > base,
            "Final weight should be higher than base"
        );
    }

    #[test]
    fn test_policy_aware_calculator_blocked() {
        let mut registry = PolicyRegistry::new();
        let mut block_policy = RoutingPolicy::new("block_test", "Block Test")
            .with_priority(100)
            .with_action("block");
        block_policy
            .filters
            .costs
            .push(model_registry::CostCategory::UltraPremium);
        registry.add(block_policy);

        let matcher = Arc::new(PolicyMatcher::new(registry));
        let config = WeightConfig::default();
        let calculator = PolicyAwareWeightCalculator::new(config, matcher);

        let auth = create_test_auth();

        // Ultra premium model should be blocked
        let expensive_model = create_test_model("expensive", "test", 60.0);
        let context = PolicyContext::default();

        let (base, factor, final_weight) = calculator.calculate_with_policy(
            &auth,
            None,
            HealthStatus::Healthy,
            &expensive_model,
            &context,
        );

        assert!(base > 0.0);
        assert!(
            factor == 0.0,
            "Blocked model should have zero policy factor"
        );
        assert!(
            final_weight == 0.0,
            "Final weight should be zero for blocked model"
        );
    }

    #[test]
    fn test_weight_calculator_factory() {
        let config = WeightConfig::default();

        // Test default factory
        let default_calc = WeightCalculatorFactory::create("weighted", config.clone(), None);
        let auth = create_test_auth();
        let weight = default_calc.calculate(&auth, None, HealthStatus::Healthy);
        assert!(weight > 0.0);

        // Test policy aware factory
        let matcher = Arc::new(PolicyMatcher::empty());
        let policy_calc = WeightCalculatorFactory::create("policy_aware", config, Some(matcher));
        let weight = policy_calc.calculate(&auth, None, HealthStatus::Healthy);
        assert!(weight > 0.0);
    }

    #[test]
    fn test_policy_weight_calculator_trait() {
        let config = WeightConfig::default();
        let matcher = Arc::new(PolicyMatcher::empty());
        let calculator = PolicyAwareWeightCalculator::new(config, matcher);

        let auth = create_test_auth();
        let model = create_test_model("test", "test", 3.0);
        let context = PolicyContext::default();

        // Test trait method
        let weight =
            calculator.calculate_with_context(&auth, None, HealthStatus::Healthy, &model, &context);
        assert!(weight > 0.0);
    }
}
