//! Credential selection with weighted random choice and policy awareness.

mod ranking;

#[cfg(test)]
mod tests;

use crate::config::SmartRoutingConfig;
use crate::health::{HealthManager, HealthStatus};
use crate::metrics::{AuthMetrics, MetricsCollector};
use crate::policy_weight::PolicyAwareWeightCalculator;
use crate::weight::{AuthInfo, WeightCalculator};
use model_registry::{PolicyMatcher, PolicyRegistry};
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

        self.calculator = match self.config.strategy.as_str() {
            "weighted" => Box::new(crate::weight::DefaultWeightCalculator::new(
                self.config.weight.clone(),
            )),
            _ => Box::new(crate::weight::DefaultWeightCalculator::new(
                self.config.weight.clone(),
            )),
        };

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
