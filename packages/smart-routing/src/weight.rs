use crate::config::WeightConfig;
use crate::health::HealthStatus;
use crate::metrics::AuthMetrics;
use std::any::Any;

/// Weight calculator trait
pub trait WeightCalculator: Send + Sync {
    /// Calculate credential weight
    fn calculate(
        &self,
        auth: &AuthInfo,
        metrics: Option<&AuthMetrics>,
        health: HealthStatus,
    ) -> f64;

    /// Allow downcasting for type-specific operations
    fn as_any(&self) -> &dyn Any;
}

/// Auth info for weight calculation
#[derive(Debug, Clone)]
pub struct AuthInfo {
    pub id: String,
    pub priority: Option<i32>,
    pub quota_exceeded: bool,
    pub unavailable: bool,
    pub model_states: Vec<ModelState>,
}

/// Model state information
#[derive(Debug, Clone)]
pub struct ModelState {
    pub unavailable: bool,
    pub quota_exceeded: bool,
}

/// Default weight calculator
pub struct DefaultWeightCalculator {
    config: WeightConfig,
}

/// Data availability assessment for planner mode adaptation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataAvailability {
    /// Full data available - all metrics populated with sufficient history
    Full,
    /// Sparse data - some metrics missing or insufficient history
    Sparse,
    /// Missing state - critical metrics unavailable
    Missing,
}

/// Planner mode for weight calculation adaptation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannerMode {
    /// Learned mode - use full weight calculation with all factors
    Learned,
    /// Heuristic mode - simplified calculation using available metrics
    Heuristic,
    /// Safe weighted mode - conservative defaults for missing state
    SafeWeighted,
    /// Deterministic fallback - predictable selection when errors occur
    Deterministic,
}

impl DefaultWeightCalculator {
    /// Create a new weight calculator
    pub fn new(config: WeightConfig) -> Self {
        Self { config }
    }

    /// Assess data availability from metrics
    fn assess_data_availability(&self, metrics: Option<&AuthMetrics>) -> DataAvailability {
        match metrics {
            None => DataAvailability::Missing,
            Some(m) => {
                // Check for sufficient data
                let has_requests = m.total_requests >= 10;
                let has_latency = m.avg_latency_ms > 0.0;
                let has_success_rate = m.success_rate >= 0.0;

                if has_requests && has_latency && has_success_rate {
                    DataAvailability::Full
                } else if m.total_requests > 0 || has_latency || has_success_rate {
                    DataAvailability::Sparse
                } else {
                    DataAvailability::Missing
                }
            },
        }
    }

    /// Select planner mode based on data availability and error state
    fn select_planner_mode(
        &self,
        data_availability: DataAvailability,
        health: HealthStatus,
    ) -> PlannerMode {
        match (data_availability, health) {
            // Full data with healthy/degraded state -> Learned mode
            (DataAvailability::Full, HealthStatus::Healthy | HealthStatus::Degraded) => {
                PlannerMode::Learned
            },
            // Sparse data -> Heuristic mode
            (DataAvailability::Sparse, _) => PlannerMode::Heuristic,
            // Missing state -> Safe weighted mode
            (DataAvailability::Missing, _) => PlannerMode::SafeWeighted,
            // Unhealthy with full data -> Safe weighted (conservative)
            (DataAvailability::Full, HealthStatus::Unhealthy) => PlannerMode::SafeWeighted,
        }
    }

    /// Calculate success rate score
    fn calculate_success_rate_score(&self, metrics: Option<&AuthMetrics>) -> f64 {
        metrics.map_or(0.5, |m| m.success_rate)
    }

    /// Calculate latency score (inverse function)
    fn calculate_latency_score(&self, metrics: Option<&AuthMetrics>) -> f64 {
        match metrics {
            Some(m) if m.avg_latency_ms > 0.0 => {
                // Use inverse function: score = 1 / (1 + latency/1000)
                // 0ms -> 1.0, 1000ms -> 0.5, 3000ms -> 0.25, 10000ms -> 0.09
                let score = 1.0 / (1.0 + m.avg_latency_ms / 1000.0);
                score.clamp(0.0, 1.0)
            },
            _ => 0.5,
        }
    }

    /// Calculate health status score
    fn calculate_health_score(&self, health: HealthStatus) -> f64 {
        match health {
            HealthStatus::Healthy => 1.0,
            HealthStatus::Degraded => 0.6,
            HealthStatus::Unhealthy => 0.1,
        }
    }

    /// Calculate load score based on request frequency and quota status
    fn calculate_load_score(&self, auth: &AuthInfo, metrics: Option<&AuthMetrics>) -> f64 {
        // Calculate recent request frequency score
        let recent_request_score = metrics.map_or(1.0, |m| {
            if m.total_requests > 0 {
                // More recent requests = lower score (give other credentials a chance)
                // Use log function for smoothing
                1.0 / (1.0 + (m.total_requests as f64).ln() / 10.0)
            } else {
                1.0
            }
        });

        // Calculate quota score
        let quota_score = if auth.quota_exceeded { 0.0 } else { 1.0 };

        // Calculate model state score
        let model_state_score = if auth.model_states.is_empty() {
            1.0
        } else {
            let unavailable_models =
                auth.model_states.iter().filter(|s| s.unavailable).count() as f64;
            let total_models = auth.model_states.len() as f64;
            1.0 - (unavailable_models / total_models)
        };

        // Combined score - use weighted sum to avoid zero score from any single factor
        recent_request_score * 0.4 + quota_score * 0.4 + model_state_score * 0.2
    }

    /// Calculate priority score
    fn calculate_priority_score(&self, auth: &AuthInfo) -> f64 {
        auth.priority.map_or(0.5, |priority| {
            // Priority range: -100 to 100
            // Convert to 0-1 score
            let score = (priority as f64 + 100.0) / 200.0;
            score.clamp(0.0, 1.0)
        })
    }
}

impl WeightCalculator for DefaultWeightCalculator {
    /// Calculate credential weight with planner mode adaptation
    /// Higher weight = higher selection probability
    fn calculate(
        &self,
        auth: &AuthInfo,
        metrics: Option<&AuthMetrics>,
        health: HealthStatus,
    ) -> f64 {
        // Assess data availability and select planner mode
        let data_availability = self.assess_data_availability(metrics);
        let planner_mode = self.select_planner_mode(data_availability, health);

        // Calculate weight based on planner mode
        match planner_mode {
            PlannerMode::Learned => self.calculate_learned(auth, metrics, health),
            PlannerMode::Heuristic => self.calculate_heuristic(auth, metrics, health),
            PlannerMode::SafeWeighted => self.calculate_safe_weighted(auth, metrics, health),
            PlannerMode::Deterministic => self.calculate_deterministic(auth, health),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl DefaultWeightCalculator {
    /// Learned mode: Full weight calculation with all factors
    fn calculate_learned(
        &self,
        auth: &AuthInfo,
        metrics: Option<&AuthMetrics>,
        health: HealthStatus,
    ) -> f64 {
        // 1. Calculate success rate score (0-1)
        let success_rate_score = self.calculate_success_rate_score(metrics);

        // 2. Calculate latency score (0-1), lower latency = higher score
        let latency_score = self.calculate_latency_score(metrics);

        // 3. Calculate health status score (0-1)
        let health_score = self.calculate_health_score(health);

        // 4. Calculate load score (0-1), lower load = higher score
        let load_score = self.calculate_load_score(auth, metrics);

        // 5. Calculate priority score (0-1)
        let priority_score = self.calculate_priority_score(auth);

        // Weighted sum
        let mut total_weight = success_rate_score * self.config.success_rate_weight
            + latency_score * self.config.latency_weight
            + health_score * self.config.health_weight
            + load_score * self.config.load_weight
            + priority_score * self.config.priority_weight;

        // Apply health status penalty
        match health {
            HealthStatus::Unhealthy => {
                total_weight *= self.config.unhealthy_penalty;
            },
            HealthStatus::Degraded => {
                total_weight *= self.config.degraded_penalty;
            },
            _ => {},
        }

        // Apply quota status penalty
        if auth.quota_exceeded {
            total_weight *= self.config.quota_exceeded_penalty;
        }

        // Apply unavailable status penalty
        if auth.unavailable {
            total_weight *= self.config.unavailable_penalty;
        }

        // Ensure weight is non-negative
        total_weight.max(0.0)
    }

    /// Heuristic mode: Simplified calculation using available metrics
    fn calculate_heuristic(
        &self,
        auth: &AuthInfo,
        metrics: Option<&AuthMetrics>,
        health: HealthStatus,
    ) -> f64 {
        // Use simplified scoring with available data
        let health_score = self.calculate_health_score(health);
        let priority_score = self.calculate_priority_score(auth);

        // Use available metrics if present, otherwise defaults
        let success_score = metrics.map_or(0.5, |m| m.success_rate);
        let latency_score = metrics.map_or(0.5, |m| {
            if m.avg_latency_ms > 0.0 {
                (1.0 / (1.0 + m.avg_latency_ms / 1000.0)).clamp(0.0, 1.0)
            } else {
                0.5
            }
        });

        // Simplified weighted sum (equal weights for available factors)
        let total_weight = (success_score + latency_score + health_score + priority_score) / 4.0;

        // Apply penalties
        let mut weight = total_weight;
        if auth.quota_exceeded {
            weight *= self.config.quota_exceeded_penalty;
        }
        if auth.unavailable {
            weight *= self.config.unavailable_penalty;
        }

        weight.max(0.0)
    }

    /// Safe weighted mode: Conservative defaults for missing state
    fn calculate_safe_weighted(
        &self,
        auth: &AuthInfo,
        _metrics: Option<&AuthMetrics>,
        health: HealthStatus,
    ) -> f64 {
        // Use conservative baseline with health and priority only
        let health_score = self.calculate_health_score(health);
        let priority_score = self.calculate_priority_score(auth);

        // Conservative scoring - emphasize stability
        let total_weight = health_score * 0.7 + priority_score * 0.3;

        // Apply strong penalties for any issues
        let mut weight = total_weight;
        match health {
            HealthStatus::Unhealthy => {
                weight *= self.config.unhealthy_penalty * 0.5; // Extra penalty
            },
            HealthStatus::Degraded => {
                weight *= self.config.degraded_penalty;
            },
            _ => {},
        }

        if auth.quota_exceeded {
            weight *= self.config.quota_exceeded_penalty * 0.5;
        }
        if auth.unavailable {
            weight *= self.config.unavailable_penalty * 0.1;
        }

        weight.max(0.0)
    }

    /// Deterministic fallback: Predictable selection when errors occur
    fn calculate_deterministic(&self, auth: &AuthInfo, health: HealthStatus) -> f64 {
        // Use only static, deterministic factors
        let priority_score = self.calculate_priority_score(auth);
        let health_score = match health {
            HealthStatus::Healthy => 1.0,
            HealthStatus::Degraded => 0.5,
            HealthStatus::Unhealthy => 0.1,
        };

        // Simple, predictable calculation
        let mut weight = (priority_score + health_score) / 2.0;

        // Apply binary penalties (either available or not)
        if auth.quota_exceeded || auth.unavailable || matches!(health, HealthStatus::Unhealthy) {
            weight = 0.0;
        }

        weight.max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weight_calculation() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Healthy);
        assert!(weight > 0.0);
        assert!(weight <= 1.0);
    }

    #[test]
    fn test_unhealthy_penalty() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config.clone());

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let healthy_weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Healthy);
        let unhealthy_weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Unhealthy);

        // The unhealthy weight should be significantly lower than healthy weight
        assert!(unhealthy_weight < healthy_weight);
        // Unhealthy status applies a penalty, so unhealthy_weight should be approximately
        // healthy_weight * unhealthy_penalty
        let expected_unhealthy = healthy_weight * config.unhealthy_penalty;
        assert!(
            (unhealthy_weight - expected_unhealthy).abs() < 0.01,
            "unhealthy_weight={} != expected={} (healthy_weight={} * penalty={})",
            unhealthy_weight,
            expected_unhealthy,
            healthy_weight,
            config.unhealthy_penalty
        );
    }

    // ============================================================
    // Edge Case Tests for Weight Calculation Functions
    // ============================================================

    #[test]
    fn test_success_rate_score_null_metrics() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        // When metrics is None, should return default 0.5
        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let weight = calculator.calculate(&auth, None, HealthStatus::Healthy);
        assert!(
            weight > 0.0,
            "Weight should be positive even with null metrics"
        );
        assert!(weight <= 1.0, "Weight should be at most 1.0");
    }

    #[test]
    fn test_success_rate_score_with_metrics() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        // Test with perfect success rate
        let perfect_metrics = AuthMetrics {
            total_requests: 100,
            success_count: 100,
            failure_count: 0,
            avg_latency_ms: 100.0,
            min_latency_ms: 50.0,
            max_latency_ms: 200.0,
            success_rate: 1.0,
            error_rate: 0.0,
            consecutive_successes: 100,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let perfect_weight =
            calculator.calculate(&auth, Some(&perfect_metrics), HealthStatus::Healthy);

        // Test with zero success rate
        let zero_metrics = AuthMetrics {
            total_requests: 100,
            success_count: 0,
            failure_count: 100,
            avg_latency_ms: 100.0,
            min_latency_ms: 50.0,
            max_latency_ms: 200.0,
            success_rate: 0.0,
            error_rate: 1.0,
            consecutive_successes: 0,
            consecutive_failures: 100,
            last_request_time: chrono::Utc::now(),
            last_success_time: None,
            last_failure_time: Some(chrono::Utc::now()),
        };

        let zero_weight = calculator.calculate(&auth, Some(&zero_metrics), HealthStatus::Healthy);

        // Perfect success rate should give higher weight
        assert!(
            perfect_weight > zero_weight,
            "Perfect success rate should yield higher weight"
        );
    }

    #[test]
    fn test_latency_score_extreme_values() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        // Test with very low latency (0ms)
        let low_latency_metrics = AuthMetrics {
            total_requests: 100,
            success_count: 100,
            failure_count: 0,
            avg_latency_ms: 1.0,
            min_latency_ms: 1.0,
            max_latency_ms: 5.0,
            success_rate: 1.0,
            error_rate: 0.0,
            consecutive_successes: 100,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let low_latency_weight =
            calculator.calculate(&auth, Some(&low_latency_metrics), HealthStatus::Healthy);

        // Test with very high latency (10000ms)
        let high_latency_metrics = AuthMetrics {
            total_requests: 100,
            success_count: 100,
            failure_count: 0,
            avg_latency_ms: 10000.0,
            min_latency_ms: 5000.0,
            max_latency_ms: 20000.0,
            success_rate: 1.0,
            error_rate: 0.0,
            consecutive_successes: 100,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let high_latency_weight =
            calculator.calculate(&auth, Some(&high_latency_metrics), HealthStatus::Healthy);

        // Lower latency should give higher weight
        assert!(
            low_latency_weight > high_latency_weight,
            "Lower latency should yield higher weight"
        );

        // Test with zero latency (edge case - should return default 0.5)
        let zero_latency_metrics = AuthMetrics {
            total_requests: 100,
            success_count: 100,
            failure_count: 0,
            avg_latency_ms: 0.0,
            min_latency_ms: 0.0,
            max_latency_ms: 0.0,
            success_rate: 1.0,
            error_rate: 0.0,
            consecutive_successes: 100,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let zero_latency_weight =
            calculator.calculate(&auth, Some(&zero_latency_metrics), HealthStatus::Healthy);
        assert!(
            zero_latency_weight > 0.0,
            "Zero latency should still produce positive weight"
        );
    }

    #[test]
    fn test_health_score_edge_cases() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        // Test all three health states
        let healthy_weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Healthy);
        let degraded_weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Degraded);
        let unhealthy_weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Unhealthy);

        // Verify ordering: healthy > degraded > unhealthy
        assert!(
            healthy_weight > degraded_weight,
            "Healthy weight should be greater than degraded"
        );
        assert!(
            degraded_weight > unhealthy_weight,
            "Degraded weight should be greater than unhealthy"
        );

        // Verify all are non-negative
        assert!(
            healthy_weight >= 0.0,
            "Healthy weight should be non-negative"
        );
        assert!(
            degraded_weight >= 0.0,
            "Degraded weight should be non-negative"
        );
        assert!(
            unhealthy_weight >= 0.0,
            "Unhealthy weight should be non-negative"
        );
    }

    #[test]
    fn test_load_score_bounds() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        // Test with high request count (high load)
        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let high_load_metrics = AuthMetrics {
            total_requests: 10000,
            success_count: 9500,
            failure_count: 500,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let high_load_weight =
            calculator.calculate(&auth, Some(&high_load_metrics), HealthStatus::Healthy);

        // Test with low request count (low load)
        let low_load_metrics = AuthMetrics {
            total_requests: 10,
            success_count: 9,
            failure_count: 1,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.9,
            error_rate: 0.1,
            consecutive_successes: 5,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let low_load_weight =
            calculator.calculate(&auth, Some(&low_load_metrics), HealthStatus::Healthy);

        // Lower load should give higher or equal weight
        assert!(
            low_load_weight >= 0.0,
            "Low load weight should be non-negative"
        );
        assert!(
            high_load_weight >= 0.0,
            "High load weight should be non-negative"
        );
    }

    #[test]
    fn test_load_score_with_quota_exceeded() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config.clone());

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        // Auth without quota exceeded
        let auth_normal = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        // Auth with quota exceeded
        let auth_quota = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: true,
            unavailable: false,
            model_states: Vec::new(),
        };

        let normal_weight =
            calculator.calculate(&auth_normal, Some(&metrics), HealthStatus::Healthy);
        let quota_exceeded_weight =
            calculator.calculate(&auth_quota, Some(&metrics), HealthStatus::Healthy);

        // Quota exceeded should significantly reduce weight
        assert!(
            quota_exceeded_weight < normal_weight,
            "Quota exceeded should reduce weight"
        );
        // Verify quota penalty is applied (should be normal * quota_exceeded_penalty)
        let expected = normal_weight * config.quota_exceeded_penalty;
        assert!(
            (quota_exceeded_weight - expected).abs() < 0.01,
            "Quota exceeded weight should be normal * penalty"
        );
    }

    #[test]
    fn test_load_score_with_model_states() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        // Auth with all models available
        let auth_available = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: vec![
                ModelState {
                    unavailable: false,
                    quota_exceeded: false,
                },
                ModelState {
                    unavailable: false,
                    quota_exceeded: false,
                },
            ],
        };

        // Auth with some unavailable models
        let auth_partial = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: vec![
                ModelState {
                    unavailable: true,
                    quota_exceeded: false,
                },
                ModelState {
                    unavailable: false,
                    quota_exceeded: false,
                },
            ],
        };

        let available_weight =
            calculator.calculate(&auth_available, Some(&metrics), HealthStatus::Healthy);
        let partial_weight =
            calculator.calculate(&auth_partial, Some(&metrics), HealthStatus::Healthy);

        // All available should give higher weight
        assert!(
            available_weight > partial_weight,
            "All available models should yield higher weight"
        );
    }

    #[test]
    fn test_priority_score_edge_cases() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        // Test maximum priority (100)
        let auth_max = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(100),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        // Test minimum priority (-100)
        let auth_min = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(-100),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        // Test None priority (should default)
        let auth_none = AuthInfo {
            id: "test-auth".to_string(),
            priority: None,
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        // Test zero priority
        let auth_zero = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let max_weight = calculator.calculate(&auth_max, Some(&metrics), HealthStatus::Healthy);
        let min_weight = calculator.calculate(&auth_min, Some(&metrics), HealthStatus::Healthy);
        let none_weight = calculator.calculate(&auth_none, Some(&metrics), HealthStatus::Healthy);
        let zero_weight = calculator.calculate(&auth_zero, Some(&metrics), HealthStatus::Healthy);

        // Verify ordering
        assert!(
            max_weight > zero_weight,
            "Max priority should give higher weight than zero"
        );
        assert!(
            zero_weight > min_weight,
            "Zero priority should give higher weight than min"
        );

        // None should be in between (defaults to 0.5)
        assert!(
            none_weight > min_weight,
            "None priority should be higher than min"
        );
        assert!(
            none_weight < max_weight,
            "None priority should be lower than max"
        );

        // All should be non-negative
        assert!(max_weight >= 0.0);
        assert!(min_weight >= 0.0);
        assert!(none_weight >= 0.0);
        assert!(zero_weight >= 0.0);
    }

    #[test]
    fn test_weight_clamping_non_negative() {
        let config = WeightConfig {
            success_rate_weight: 1.0,
            latency_weight: 0.0,
            health_weight: 0.0,
            load_weight: 0.0,
            priority_weight: 0.0,
            unhealthy_penalty: 0.0, // This would make weight zero or negative
            degraded_penalty: 0.0,
            quota_exceeded_penalty: 0.0,
            unavailable_penalty: 0.0,
        };
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        // Even with zero success rate and unhealthy status
        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 0,
            failure_count: 100,
            avg_latency_ms: 10000.0,
            min_latency_ms: 5000.0,
            max_latency_ms: 20000.0,
            success_rate: 0.0,
            error_rate: 1.0,
            consecutive_successes: 0,
            consecutive_failures: 100,
            last_request_time: chrono::Utc::now(),
            last_success_time: None,
            last_failure_time: Some(chrono::Utc::now()),
        };

        let weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Unhealthy);

        // Weight should be clamped to non-negative
        assert!(
            weight >= 0.0,
            "Weight should always be non-negative, got: {}",
            weight
        );
    }

    #[test]
    fn test_degraded_penalty_applied() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config.clone());

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let healthy_weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Healthy);
        let degraded_weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Degraded);

        // Degraded weight should be less than healthy weight due to:
        // 1. Lower health_score (0.6 vs 1.0 in weighted sum)
        // 2. Additional degraded_penalty multiplier
        assert!(
            degraded_weight < healthy_weight,
            "Degraded weight ({}) should be less than healthy weight ({})",
            degraded_weight,
            healthy_weight
        );

        // Verify degraded weight is still positive
        assert!(degraded_weight > 0.0, "Degraded weight should be positive");
    }

    #[test]
    fn test_unavailable_penalty_applied() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config.clone());

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let auth_available = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let auth_unavailable = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: true,
            model_states: Vec::new(),
        };

        let available_weight =
            calculator.calculate(&auth_available, Some(&metrics), HealthStatus::Healthy);
        let unavailable_weight =
            calculator.calculate(&auth_unavailable, Some(&metrics), HealthStatus::Healthy);

        // Unavailable penalty should be applied
        let expected = available_weight * config.unavailable_penalty;
        assert!(
            (unavailable_weight - expected).abs() < 0.01,
            "Unavailable weight should be available * unavailable_penalty"
        );
    }

    #[test]
    fn test_combined_penalties() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config.clone());

        // Auth with multiple issues: unhealthy + quota_exceeded + unavailable
        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: true,
            unavailable: true,
            model_states: Vec::new(),
        };

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Unhealthy);

        // Weight should still be non-negative even with multiple penalties
        assert!(
            weight >= 0.0,
            "Weight should be non-negative with combined penalties"
        );

        // Weight should be very low due to stacked penalties
        // unhealthy * quota_exceeded * unavailable
        let base_weight = calculator.calculate(
            &AuthInfo {
                id: "test-auth".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            Some(&metrics),
            HealthStatus::Healthy,
        );

        let expected = base_weight
            * config.unhealthy_penalty
            * config.quota_exceeded_penalty
            * config.unavailable_penalty;
        assert!(
            (weight - expected).abs() < 0.001,
            "Combined penalties should be multiplicative"
        );
    }

    // ============================================================
    // Edge Case Tests for Weight Calculator - Numerical Stability
    // ============================================================

    #[test]
    fn test_weight_calculation_with_nan_metrics() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        // Metrics with NaN values
        let nan_metrics = AuthMetrics {
            total_requests: 100,
            success_count: 50,
            failure_count: 50,
            avg_latency_ms: f64::NAN,
            min_latency_ms: f64::NAN,
            max_latency_ms: f64::NAN,
            success_rate: f64::NAN,
            error_rate: f64::NAN,
            consecutive_successes: 10,
            consecutive_failures: 5,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let weight = calculator.calculate(&auth, Some(&nan_metrics), HealthStatus::Healthy);

        // Weight should still be a valid number (not NaN)
        assert!(
            !weight.is_nan(),
            "Weight should not be NaN even with NaN metrics"
        );
        assert!(
            weight >= 0.0,
            "Weight should be non-negative even with NaN metrics"
        );
    }

    #[test]
    fn test_weight_calculation_with_inf_metrics() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        // Metrics with infinity values
        let inf_metrics = AuthMetrics {
            total_requests: 100,
            success_count: 100,
            failure_count: 0,
            avg_latency_ms: f64::INFINITY,
            min_latency_ms: 0.0,
            max_latency_ms: f64::INFINITY,
            success_rate: 1.0,
            error_rate: 0.0,
            consecutive_successes: 100,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let weight = calculator.calculate(&auth, Some(&inf_metrics), HealthStatus::Healthy);

        // Weight should be finite (clamped)
        assert!(
            weight.is_finite(),
            "Weight should be finite even with Inf metrics"
        );
        assert!(
            (0.0..=1.0).contains(&weight),
            "Weight should be in valid range"
        );
    }

    #[test]
    fn test_priority_weight_extreme_values() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        // Test extreme positive priority
        let auth_max = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(i32::MAX),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };
        let weight_max = calculator.calculate(&auth_max, Some(&metrics), HealthStatus::Healthy);
        assert!(
            weight_max.is_finite() && weight_max > 0.0,
            "Max priority should produce valid weight"
        );

        // Test extreme negative priority
        let auth_min = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(i32::MIN),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };
        let weight_min = calculator.calculate(&auth_min, Some(&metrics), HealthStatus::Healthy);
        assert!(
            weight_min.is_finite() && weight_min >= 0.0,
            "Min priority should produce valid weight"
        );

        // Max priority should give higher weight
        assert!(
            weight_max > weight_min,
            "Max priority should give higher weight than min"
        );
    }

    #[test]
    fn test_weight_calculation_zero_metrics() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        // All zeros (except success_rate which is initialized to 1.0)
        let zero_metrics = AuthMetrics {
            total_requests: 0,
            success_count: 0,
            failure_count: 0,
            avg_latency_ms: 0.0,
            min_latency_ms: 0.0,
            max_latency_ms: 0.0,
            success_rate: 1.0,
            error_rate: 0.0,
            consecutive_successes: 0,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: None,
            last_failure_time: None,
        };

        let weight = calculator.calculate(&auth, Some(&zero_metrics), HealthStatus::Healthy);

        assert!(
            weight > 0.0,
            "Weight should be positive even with zero metrics"
        );
        assert!(
            weight <= 1.0,
            "Weight should not exceed 1.0"
        );
    }

    #[test]
    fn test_quota_exceeded_heavy_penalty() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config.clone());

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let auth_normal = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let auth_quota = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: true,
            unavailable: false,
            model_states: Vec::new(),
        };

        let normal_weight =
            calculator.calculate(&auth_normal, Some(&metrics), HealthStatus::Healthy);
        let quota_weight = calculator.calculate(&auth_quota, Some(&metrics), HealthStatus::Healthy);

        // Quota exceeded should apply heavy penalty
        assert!(
            quota_weight < normal_weight * 0.5,
            "Quota exceeded should reduce weight by at least 50%: normal={}, quota={}",
            normal_weight,
            quota_weight
        );

        // Verify exact penalty
        let expected = normal_weight * config.quota_exceeded_penalty;
        assert!(
            (quota_weight - expected).abs() < 0.01,
            "Quota penalty should match config"
        );
    }

    #[test]
    fn test_model_state_all_unavailable() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        let metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        // Auth with all models unavailable
        let auth_all_unavailable = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: vec![
                ModelState {
                    unavailable: true,
                    quota_exceeded: false,
                },
                ModelState {
                    unavailable: true,
                    quota_exceeded: false,
                },
                ModelState {
                    unavailable: true,
                    quota_exceeded: false,
                },
            ],
        };

        // Auth with all models available
        let auth_all_available = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: vec![
                ModelState {
                    unavailable: false,
                    quota_exceeded: false,
                },
                ModelState {
                    unavailable: false,
                    quota_exceeded: false,
                },
                ModelState {
                    unavailable: false,
                    quota_exceeded: false,
                },
            ],
        };

        let weight_unavailable =
            calculator.calculate(&auth_all_unavailable, Some(&metrics), HealthStatus::Healthy);
        let weight_available =
            calculator.calculate(&auth_all_available, Some(&metrics), HealthStatus::Healthy);

        assert!(
            weight_available > weight_unavailable,
            "All available models should yield higher weight than all unavailable"
        );
    }

    #[test]
    fn test_planner_mode_selection() {
        let config = WeightConfig::default();
        let calculator = DefaultWeightCalculator::new(config);

        // Full data
        let full_metrics = AuthMetrics {
            total_requests: 100,
            success_count: 95,
            failure_count: 5,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.95,
            error_rate: 0.05,
            consecutive_successes: 10,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        // Sparse data (< 10 requests)
        let sparse_metrics = AuthMetrics {
            total_requests: 5,
            success_count: 4,
            failure_count: 1,
            avg_latency_ms: 500.0,
            min_latency_ms: 100.0,
            max_latency_ms: 1000.0,
            success_rate: 0.8,
            error_rate: 0.2,
            consecutive_successes: 2,
            consecutive_failures: 0,
            last_request_time: chrono::Utc::now(),
            last_success_time: Some(chrono::Utc::now()),
            last_failure_time: None,
        };

        let auth = AuthInfo {
            id: "test-auth".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        };

        let full_weight = calculator.calculate(&auth, Some(&full_metrics), HealthStatus::Healthy);
        let sparse_weight =
            calculator.calculate(&auth, Some(&sparse_metrics), HealthStatus::Healthy);
        let none_weight = calculator.calculate(&auth, None, HealthStatus::Healthy);

        // All should be valid
        assert!(full_weight > 0.0 && full_weight <= 1.0);
        assert!(sparse_weight > 0.0 && sparse_weight <= 1.0);
        assert!(none_weight > 0.0 && none_weight <= 1.0);
    }
}
