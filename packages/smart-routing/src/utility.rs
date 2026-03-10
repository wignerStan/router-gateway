use crate::metrics::AuthMetrics;
use serde::{Deserialize, Serialize};

/// Utility estimator for route quality assessment
///
/// Utility combines success rate, latency, and cost into a single score
/// for ranking and selecting routes.
#[derive(Debug, Clone)]
pub struct UtilityEstimator {
    config: UtilityConfig,
}

/// Utility estimation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtilityConfig {
    /// Success rate utility weight (0-1)
    pub success_weight: f64,
    /// Latency utility weight (0-1)
    pub latency_weight: f64,
    /// Cost utility weight (0-1)
    pub cost_weight: f64,
    /// Cost sensitivity: how much cost impacts utility
    pub cost_sensitivity: f64,
    /// Latency normalization factor (ms)
    pub latency_normalization_ms: f64,
    /// Minimum utility floor for new/untested routes
    pub min_utility: f64,
}

impl Default for UtilityConfig {
    fn default() -> Self {
        Self {
            success_weight: 0.5,
            latency_weight: 0.3,
            cost_weight: 0.2,
            cost_sensitivity: 1.0,
            latency_normalization_ms: 1000.0,
            min_utility: 0.1,
        }
    }
}

impl Default for UtilityEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl UtilityEstimator {
    /// Create a new utility estimator with default config
    pub fn new() -> Self {
        Self {
            config: UtilityConfig::default(),
        }
    }

    /// Create a new utility estimator with custom config
    pub fn with_config(config: UtilityConfig) -> Self {
        Self { config }
    }

    /// Estimate utility from metrics
    ///
    /// Higher utility = better route. Returns value in [0, 1].
    pub fn estimate_utility(&self, metrics: Option<&AuthMetrics>) -> f64 {
        match metrics {
            None => self.config.min_utility,
            Some(m) => {
                if m.total_requests == 0 {
                    return self.config.min_utility;
                }

                let success_utility = self.estimate_success_utility(m);
                let latency_utility = self.estimate_latency_utility(m);
                let cost_utility = self.estimate_cost_utility(m);

                // Weighted combination
                let utility = success_utility * self.config.success_weight
                    + latency_utility * self.config.latency_weight
                    + cost_utility * self.config.cost_weight;

                utility.max(self.config.min_utility)
            },
        }
    }

    /// Estimate utility with explicit cost parameter
    pub fn estimate_utility_with_cost(
        &self,
        metrics: Option<&AuthMetrics>,
        cost_per_million: f64,
    ) -> f64 {
        match metrics {
            None => self.config.min_utility,
            Some(m) => {
                if m.total_requests == 0 {
                    return self.config.min_utility;
                }

                let success_utility = self.estimate_success_utility(m);
                let latency_utility = self.estimate_latency_utility(m);
                let cost_utility = self.estimate_cost_utility_with_cost(cost_per_million);

                // Weighted combination
                let utility = success_utility * self.config.success_weight
                    + latency_utility * self.config.latency_weight
                    + cost_utility * self.config.cost_weight;

                utility.max(self.config.min_utility)
            },
        }
    }

    /// Estimate success utility (0-1)
    /// High success rate -> high utility
    fn estimate_success_utility(&self, metrics: &AuthMetrics) -> f64 {
        // Use success_rate directly (it's already EWMA smoothed)
        metrics.success_rate.clamp(0.0, 1.0)
    }

    /// Estimate latency utility (0-1)
    /// Low latency -> high utility
    fn estimate_latency_utility(&self, metrics: &AuthMetrics) -> f64 {
        if metrics.avg_latency_ms <= 0.0 {
            return 1.0; // No latency data = optimistic
        }

        // Use inverse function: higher latency = lower utility
        // utility = 1 / (1 + latency / normalization)
        let utility = 1.0 / (1.0 + metrics.avg_latency_ms / self.config.latency_normalization_ms);
        utility.clamp(0.0, 1.0)
    }

    /// Estimate cost utility (0-1) from cost per million tokens
    /// Low cost -> high utility
    fn estimate_cost_utility_with_cost(&self, cost_per_million: f64) -> f64 {
        if cost_per_million <= 0.0 {
            return 1.0; // Free = maximum utility
        }

        // Use inverse function with sensitivity
        // utility = 1 / (1 + cost * sensitivity)
        let utility = 1.0 / (1.0 + cost_per_million * self.config.cost_sensitivity / 10.0);
        utility.clamp(0.0, 1.0)
    }

    /// Estimate cost utility (0-1)
    /// Low cost -> high utility (not used if cost not available)
    fn estimate_cost_utility(&self, _metrics: &AuthMetrics) -> f64 {
        // Default cost utility when cost not available
        1.0
    }

    /// Get config
    pub fn config(&self) -> &UtilityConfig {
        &self.config
    }

    /// Set config
    pub fn set_config(&mut self, config: UtilityConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_metrics(success_rate: f64, avg_latency_ms: f64) -> AuthMetrics {
        AuthMetrics {
            total_requests: 100,
            success_count: (success_rate * 100.0) as i64,
            failure_count: ((1.0 - success_rate) * 100.0) as i64,
            avg_latency_ms,
            min_latency_ms: avg_latency_ms * 0.5,
            max_latency_ms: avg_latency_ms * 1.5,
            success_rate,
            error_rate: 1.0 - success_rate,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: Utc::now(),
            last_success_time: Some(Utc::now()),
            last_failure_time: None,
        }
    }

    #[test]
    fn test_utility_estimation_high_success() {
        let estimator = UtilityEstimator::new();
        let metrics = create_metrics(0.95, 500.0);
        let utility = estimator.estimate_utility(Some(&metrics));

        // High success rate should give high utility
        assert!(utility > 0.6, "High success should yield high utility");
    }

    #[test]
    fn test_utility_estimation_low_success() {
        let estimator = UtilityEstimator::new();
        let metrics = create_metrics(0.2, 500.0);
        let utility = estimator.estimate_utility(Some(&metrics));

        // Low success rate should give low utility
        // With success=0.2, latency=500: utility = 0.2*0.5 + 0.667*0.3 + 1.0*0.2 = 0.5
        assert!(utility <= 0.5, "Low success should yield utility <= 0.5");
    }

    #[test]
    fn test_utility_estimation_low_latency() {
        let estimator = UtilityEstimator::new();
        let metrics = create_metrics(0.8, 100.0);
        let utility = estimator.estimate_utility(Some(&metrics));

        // Low latency should give higher utility
        assert!(utility > 0.6, "Low latency should yield high utility");
    }

    #[test]
    fn test_utility_estimation_high_latency() {
        let estimator = UtilityEstimator::new();
        let metrics = create_metrics(0.8, 5000.0);
        let utility = estimator.estimate_utility(Some(&metrics));

        // High latency should give lower utility
        // With success=0.8, latency=5000: utility = 0.8*0.5 + 0.167*0.3 + 1.0*0.2 = 0.65
        assert!(utility < 0.7, "High latency should yield lower utility");
    }

    #[test]
    fn test_utility_estimation_with_cost() {
        let estimator = UtilityEstimator::new();
        let metrics = create_metrics(0.8, 500.0);

        // Low cost should give higher utility
        let low_cost_utility = estimator.estimate_utility_with_cost(Some(&metrics), 1.0);
        let high_cost_utility = estimator.estimate_utility_with_cost(Some(&metrics), 100.0);

        assert!(
            low_cost_utility > high_cost_utility,
            "Low cost should yield higher utility"
        );
    }

    #[test]
    fn test_utility_estimation_no_metrics() {
        let estimator = UtilityEstimator::new();
        let utility = estimator.estimate_utility(None);

        // No metrics should return minimum utility
        assert_eq!(utility, estimator.config.min_utility);
    }

    #[test]
    fn test_utility_estimation_zero_requests() {
        let estimator = UtilityEstimator::new();
        let metrics = AuthMetrics {
            total_requests: 0,
            success_count: 0,
            failure_count: 0,
            avg_latency_ms: 0.0,
            min_latency_ms: f64::MAX,
            max_latency_ms: 0.0,
            success_rate: 1.0,
            error_rate: 0.0,
            consecutive_successes: 0,
            consecutive_failures: 0,
            last_request_time: Utc::now(),
            last_success_time: None,
            last_failure_time: None,
        };

        let utility = estimator.estimate_utility(Some(&metrics));
        assert_eq!(utility, estimator.config.min_utility);
    }

    #[test]
    fn test_latency_utility_clamping() {
        let estimator = UtilityEstimator::new();

        // Very high latency should still produce positive utility
        let metrics = create_metrics(0.8, 100000.0);
        let utility = estimator.estimate_utility(Some(&metrics));
        assert!(utility >= 0.0, "Utility should be non-negative");
    }

    #[test]
    fn test_success_utility_bounds() {
        let estimator = UtilityEstimator::new();

        // Success rate at bounds
        let metrics_high = create_metrics(1.0, 500.0);
        let utility_high = estimator.estimate_utility(Some(&metrics_high));

        let metrics_low = create_metrics(0.0, 500.0);
        let utility_low = estimator.estimate_utility(Some(&metrics_low));

        assert!(
            utility_high > utility_low,
            "Higher success should give higher utility"
        );
    }

    #[test]
    fn test_cost_sensitivity() {
        let config = UtilityConfig {
            cost_sensitivity: 2.0, // High sensitivity
            ..UtilityConfig::default()
        };

        let estimator = UtilityEstimator::with_config(config);
        let metrics = create_metrics(0.8, 500.0);

        // High cost sensitivity amplifies cost differences
        // cost=1: utility = 0.8*0.5 + 0.667*0.3 + 0.833*0.2 = 0.767
        // cost=100: utility = 0.8*0.5 + 0.667*0.3 + 0.048*0.2 = 0.610
        let low_cost_utility = estimator.estimate_utility_with_cost(Some(&metrics), 1.0);
        let high_cost_utility = estimator.estimate_utility_with_cost(Some(&metrics), 100.0);

        assert!(
            (low_cost_utility - high_cost_utility) > 0.1,
            "High cost sensitivity should amplify utility difference (diff = {})",
            low_cost_utility - high_cost_utility
        );
    }
}
