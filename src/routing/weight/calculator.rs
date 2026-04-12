use super::types::{AuthInfo, DataAvailability, PlannerMode};
use crate::routing::config::WeightConfig;
use crate::routing::health::HealthStatus;
use crate::routing::metrics::AuthMetrics;
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

/// Default weight calculator
pub struct DefaultWeightCalculator {
    config: WeightConfig,
}

impl DefaultWeightCalculator {
    /// Create a new weight calculator with the given configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::routing::{DefaultWeightCalculator, WeightCalculator, WeightConfig, AuthInfo, HealthStatus};
    ///
    /// let calc = DefaultWeightCalculator::new(WeightConfig::default());
    ///
    /// let auth = AuthInfo {
    ///     id: "cred-1".to_string(),
    ///     priority: Some(0),
    ///     quota_exceeded: false,
    ///     unavailable: false,
    ///     model_states: vec![],
    /// };
    ///
    /// let weight = calc.calculate(&auth, None, HealthStatus::Healthy);
    /// assert!(weight > 0.0, "healthy credential should have positive weight");
    /// ```
    #[must_use]
    pub const fn new(config: WeightConfig) -> Self {
        Self { config }
    }

    /// Assess data availability from metrics
    fn assess_data_availability(metrics: Option<&AuthMetrics>) -> DataAvailability {
        metrics.map_or(DataAvailability::Missing, |m| {
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
        })
    }

    /// Select planner mode based on data availability and error state
    const fn select_planner_mode(
        data_availability: DataAvailability,
        health: HealthStatus,
    ) -> PlannerMode {
        match (data_availability, health) {
            (DataAvailability::Full, HealthStatus::Healthy | HealthStatus::Degraded) => {
                PlannerMode::Learned
            },
            (DataAvailability::Sparse, _) => PlannerMode::Heuristic,
            (DataAvailability::Missing, HealthStatus::Healthy | HealthStatus::Degraded) => {
                PlannerMode::SafeWeighted
            },
            (DataAvailability::Full, HealthStatus::Unhealthy) => PlannerMode::SafeWeighted,
            (DataAvailability::Missing, HealthStatus::Unhealthy) => PlannerMode::Deterministic,
        }
    }

    // ---- Scoring functions ----

    /// Calculate success rate score
    fn calculate_success_rate_score(metrics: Option<&AuthMetrics>) -> f64 {
        metrics.map_or(0.5, |m| {
            if m.success_rate.is_finite() {
                m.success_rate
            } else {
                0.5
            }
        })
    }

    /// Calculate latency score (inverse function)
    fn calculate_latency_score(metrics: Option<&AuthMetrics>) -> f64 {
        match metrics {
            Some(m) if m.avg_latency_ms > 0.0 && m.avg_latency_ms.is_finite() => {
                let score = 1.0 / (1.0 + m.avg_latency_ms / 1000.0);
                score.clamp(0.0, 1.0)
            },
            _ => 0.5,
        }
    }

    /// Calculate health status score
    const fn calculate_health_score(health: HealthStatus) -> f64 {
        match health {
            HealthStatus::Healthy => 1.0,
            HealthStatus::Degraded => 0.6,
            HealthStatus::Unhealthy => 0.1,
        }
    }

    /// Calculate load score based on request frequency and quota status
    fn calculate_load_score(auth: &AuthInfo, metrics: Option<&AuthMetrics>) -> f64 {
        let recent_request_score = metrics.map_or(1.0, |m| {
            if m.total_requests > 0 {
                1.0 / (1.0 + (m.total_requests as f64).ln() / 10.0)
            } else {
                1.0
            }
        });

        let quota_score = if auth.quota_exceeded { 0.0 } else { 1.0 };

        let model_state_score = if auth.model_states.is_empty() {
            1.0
        } else {
            let unavailable_models =
                auth.model_states.iter().filter(|s| s.unavailable).count() as f64;
            let total_models = auth.model_states.len() as f64;
            1.0 - (unavailable_models / total_models)
        };

        recent_request_score * 0.4 + quota_score * 0.4 + model_state_score * 0.2
    }

    /// Calculate priority score
    fn calculate_priority_score(auth: &AuthInfo) -> f64 {
        auth.priority.map_or(0.5, |priority| {
            let score = (f64::from(priority) + 100.0) / 200.0;
            score.clamp(0.0, 1.0)
        })
    }

    // ---- Mode-specific calculators ----

    /// Learned mode: Full weight calculation with all factors
    fn calculate_learned(
        &self,
        auth: &AuthInfo,
        metrics: Option<&AuthMetrics>,
        health: HealthStatus,
    ) -> f64 {
        let success_rate_score = Self::calculate_success_rate_score(metrics);
        let latency_score = Self::calculate_latency_score(metrics);
        let health_score = Self::calculate_health_score(health);
        let load_score = Self::calculate_load_score(auth, metrics);
        let priority_score = Self::calculate_priority_score(auth);

        let mut total_weight = priority_score.mul_add(
            self.config.priority_weight,
            load_score.mul_add(
                self.config.load_weight,
                health_score.mul_add(
                    self.config.health_weight,
                    success_rate_score.mul_add(
                        self.config.success_rate_weight,
                        latency_score * self.config.latency_weight,
                    ),
                ),
            ),
        );

        match health {
            HealthStatus::Unhealthy => {
                total_weight *= self.config.unhealthy_penalty;
            },
            HealthStatus::Degraded => {
                total_weight *= self.config.degraded_penalty;
            },
            HealthStatus::Healthy => {},
        }

        if auth.quota_exceeded {
            total_weight *= self.config.quota_exceeded_penalty;
        }

        if auth.unavailable {
            total_weight *= self.config.unavailable_penalty;
        }

        total_weight.max(0.0)
    }

    /// Heuristic mode: Simplified calculation using available metrics
    fn calculate_heuristic(
        &self,
        auth: &AuthInfo,
        metrics: Option<&AuthMetrics>,
        health: HealthStatus,
    ) -> f64 {
        let health_score = Self::calculate_health_score(health);
        let priority_score = Self::calculate_priority_score(auth);

        let success_score = metrics.map_or(0.5, |m| m.success_rate);
        let latency_score = metrics.map_or(0.5, |m| {
            if m.avg_latency_ms > 0.0 {
                (1.0 / (1.0 + m.avg_latency_ms / 1000.0)).clamp(0.0, 1.0)
            } else {
                0.5
            }
        });

        let total_weight = (success_score + latency_score + health_score + priority_score) / 4.0;

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
        let health_score = Self::calculate_health_score(health);
        let priority_score = Self::calculate_priority_score(auth);

        let total_weight = health_score.mul_add(0.7, priority_score * 0.3);

        let mut weight = total_weight;
        match health {
            HealthStatus::Unhealthy => {
                weight *= self.config.unhealthy_penalty * 0.5;
            },
            HealthStatus::Degraded => {
                weight *= self.config.degraded_penalty;
            },
            HealthStatus::Healthy => {},
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
    fn calculate_deterministic(auth: &AuthInfo, health: HealthStatus) -> f64 {
        let priority_score = Self::calculate_priority_score(auth);
        let health_score = Self::calculate_health_score(health);

        if auth.quota_exceeded || auth.unavailable || matches!(health, HealthStatus::Unhealthy) {
            return 0.0;
        }

        f64::midpoint(priority_score, health_score).max(0.0)
    }
}

impl WeightCalculator for DefaultWeightCalculator {
    /// Calculate credential weight with planner mode adaptation.
    /// Higher weight = higher selection probability.
    fn calculate(
        &self,
        auth: &AuthInfo,
        metrics: Option<&AuthMetrics>,
        health: HealthStatus,
    ) -> f64 {
        let data_availability = Self::assess_data_availability(metrics);
        let planner_mode = Self::select_planner_mode(data_availability, health);

        match planner_mode {
            PlannerMode::Learned => self.calculate_learned(auth, metrics, health),
            PlannerMode::Heuristic => self.calculate_heuristic(auth, metrics, health),
            PlannerMode::SafeWeighted => self.calculate_safe_weighted(auth, metrics, health),
            PlannerMode::Deterministic => Self::calculate_deterministic(auth, health),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
