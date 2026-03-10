use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Smart routing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartRoutingConfig {
    /// Whether smart routing is enabled
    pub enabled: bool,
    /// Routing strategy: weighted, time_aware, quota_aware, adaptive, policy_aware
    pub strategy: String,
    /// Weight configuration
    pub weight: WeightConfig,
    /// Health check configuration
    pub health: HealthConfig,
    /// Time-aware configuration
    pub time_aware: TimeAwareConfig,
    /// Quota-aware configuration
    pub quota_aware: QuotaAwareConfig,
    /// Policy-aware configuration
    #[serde(default)]
    pub policy: PolicyConfig,
    /// Log configuration
    pub log: LogConfig,
}

/// Policy-based routing configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolicyConfig {
    /// Whether policy-aware routing is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Path to policy configuration file (JSON)
    #[serde(default)]
    pub config_path: Option<String>,

    /// Inline policy definitions (alternative to config file)
    #[serde(default)]
    pub inline_policies: Vec<model_registry::RoutingPolicy>,

    /// Cache policy evaluation results for performance
    #[serde(default = "default_cache_enabled")]
    pub cache_enabled: bool,
}

fn default_cache_enabled() -> bool {
    true
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            config_path: None,
            inline_policies: Vec::new(),
            cache_enabled: true,
        }
    }
}

/// Weight calculation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightConfig {
    /// Success rate weight (0-1)
    pub success_rate_weight: f64,
    /// Latency weight (0-1)
    pub latency_weight: f64,
    /// Health status weight (0-1)
    pub health_weight: f64,
    /// Load weight (0-1)
    pub load_weight: f64,
    /// Priority weight (0-1)
    pub priority_weight: f64,
    /// Penalty factor for unhealthy credentials (default: 0.01)
    pub unhealthy_penalty: f64,
    /// Penalty factor for degraded credentials (default: 0.5)
    pub degraded_penalty: f64,
    /// Penalty factor for quota-exceeded credentials (default: 0.1)
    pub quota_exceeded_penalty: f64,
    /// Penalty factor for unavailable credentials (default: 0.01)
    pub unavailable_penalty: f64,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthConfig {
    /// Healthy threshold: consecutive successes to recover
    pub healthy_threshold: i32,
    /// Unhealthy threshold: consecutive failures to mark unhealthy
    pub unhealthy_threshold: i32,
    /// Degraded threshold: error rate above this enters degraded state
    pub degraded_threshold: f64,
    /// Cooldown period: wait time after failure (seconds)
    pub cooldown_period_seconds: i64,
    /// HTTP status code health rules
    pub status_codes: StatusCodeHealthConfig,
}

/// HTTP status code health configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusCodeHealthConfig {
    /// Successful status codes
    pub healthy: Vec<i32>,
    /// Degraded status codes
    pub degraded: Vec<i32>,
    /// Unhealthy status codes
    pub unhealthy: Vec<i32>,
}

/// Time-aware configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeAwareConfig {
    /// Whether time-aware routing is enabled
    pub enabled: bool,
    /// Peak hours configuration
    pub peak_hours: Vec<TimeSlot>,
    /// Off-peak weight factor
    pub off_peak_factor: f64,
    /// Preferred credentials per time slot
    pub preferred_auths_per_time_slot: HashMap<String, Vec<String>>,
}

/// Time slot definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSlot {
    /// Start hour (0-23)
    pub start_hour: i32,
    /// End hour (0-23)
    pub end_hour: i32,
    /// Days of week (0=Sunday, 1-6=Monday-Saturday)
    pub days_of_week: Vec<i32>,
    /// Weight factor
    pub factor: f64,
}

/// Quota-aware configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaAwareConfig {
    /// Whether quota-aware routing is enabled
    pub enabled: bool,
    /// Quota balance strategy: least_used, round_robin, adaptive
    pub quota_balance_strategy: String,
    /// Reserve ratio: quota reserved for peak periods
    pub reserve_ratio: f64,
    /// Recovery window: quota recovery prediction window (seconds)
    pub recovery_window_seconds: i64,
}

/// Log configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    /// Whether to log routing decisions
    pub enabled: bool,
    /// Log level: debug, info, warn, error
    pub level: String,
}

impl Default for SmartRoutingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            strategy: "weighted".to_string(),
            weight: WeightConfig::default(),
            health: HealthConfig::default(),
            time_aware: TimeAwareConfig::default(),
            quota_aware: QuotaAwareConfig::default(),
            policy: PolicyConfig::default(),
            log: LogConfig::default(),
        }
    }
}

impl Default for WeightConfig {
    fn default() -> Self {
        Self {
            success_rate_weight: 0.35,
            latency_weight: 0.25,
            health_weight: 0.20,
            load_weight: 0.15,
            priority_weight: 0.05,
            unhealthy_penalty: 0.01,
            degraded_penalty: 0.5,
            quota_exceeded_penalty: 0.1,
            unavailable_penalty: 0.01,
        }
    }
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            healthy_threshold: 3,
            unhealthy_threshold: 5,
            degraded_threshold: 0.3,
            cooldown_period_seconds: 60,
            status_codes: StatusCodeHealthConfig::default(),
        }
    }
}

impl Default for StatusCodeHealthConfig {
    fn default() -> Self {
        Self {
            healthy: vec![200, 201, 202, 204],
            degraded: vec![429, 503],
            unhealthy: vec![401, 402, 403, 500, 502, 504],
        }
    }
}

impl Default for TimeAwareConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            peak_hours: Vec::new(),
            off_peak_factor: 1.2,
            preferred_auths_per_time_slot: HashMap::new(),
        }
    }
}

impl Default for QuotaAwareConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            quota_balance_strategy: "adaptive".to_string(),
            reserve_ratio: 0.2,
            recovery_window_seconds: 3600,
        }
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            level: "info".to_string(),
        }
    }
}

impl SmartRoutingConfig {
    /// Normalize configuration to ensure weights sum to 1
    pub fn normalize(&mut self) {
        let total_weight = self.weight.success_rate_weight
            + self.weight.latency_weight
            + self.weight.health_weight
            + self.weight.load_weight
            + self.weight.priority_weight;

        if total_weight > 0.0 && (total_weight - 1.0).abs() > f64::EPSILON {
            self.weight.success_rate_weight /= total_weight;
            self.weight.latency_weight /= total_weight;
            self.weight.health_weight /= total_weight;
            self.weight.load_weight /= total_weight;
            self.weight.priority_weight /= total_weight;
        }

        // Ensure thresholds are valid
        if self.health.healthy_threshold <= 0 {
            self.health.healthy_threshold = 3;
        }
        if self.health.unhealthy_threshold <= 0 {
            self.health.unhealthy_threshold = 5;
        }
        if self.health.degraded_threshold <= 0.0 || self.health.degraded_threshold > 1.0 {
            self.health.degraded_threshold = 0.3;
        }
        if self.health.cooldown_period_seconds <= 0 {
            self.health.cooldown_period_seconds = 60;
        }

        // Ensure quota parameters are valid
        if self.quota_aware.reserve_ratio < 0.0 || self.quota_aware.reserve_ratio > 1.0 {
            self.quota_aware.reserve_ratio = 0.2;
        }
        if self.quota_aware.recovery_window_seconds <= 0 {
            self.quota_aware.recovery_window_seconds = 3600;
        }
    }

    /// Validate configuration
    pub fn validate(&mut self) -> Result<(), String> {
        // Validate strategy
        const VALID_STRATEGIES: &[&str] = &["weighted", "time_aware", "quota_aware", "adaptive"];
        if !VALID_STRATEGIES.contains(&self.strategy.as_str()) {
            self.strategy = "weighted".to_string();
        }

        // Validate weight ranges
        if self.weight.success_rate_weight < 0.0 || self.weight.success_rate_weight > 1.0 {
            self.weight.success_rate_weight = 0.35;
        }
        if self.weight.latency_weight < 0.0 || self.weight.latency_weight > 1.0 {
            self.weight.latency_weight = 0.25;
        }
        if self.weight.health_weight < 0.0 || self.weight.health_weight > 1.0 {
            self.weight.health_weight = 0.20;
        }
        if self.weight.load_weight < 0.0 || self.weight.load_weight > 1.0 {
            self.weight.load_weight = 0.15;
        }
        if self.weight.priority_weight < 0.0 || self.weight.priority_weight > 1.0 {
            self.weight.priority_weight = 0.05;
        }

        // Validate time-aware config
        if self.time_aware.off_peak_factor <= 0.0 {
            self.time_aware.off_peak_factor = 1.2;
        }

        // Validate time slots
        for slot in &mut self.time_aware.peak_hours {
            slot.validate();
        }

        // Validate quota balance strategy
        const VALID_QUOTA_STRATEGIES: &[&str] = &["least_used", "round_robin", "adaptive"];
        if !VALID_QUOTA_STRATEGIES.contains(&self.quota_aware.quota_balance_strategy.as_str()) {
            self.quota_aware.quota_balance_strategy = "adaptive".to_string();
        }

        self.normalize();
        Ok(())
    }

    /// Create a deep copy of the configuration
    pub fn clone_config(&self) -> Self {
        Self {
            enabled: self.enabled,
            strategy: self.strategy.clone(),
            weight: self.weight.clone(),
            health: self.health.clone(),
            time_aware: TimeAwareConfig {
                enabled: self.time_aware.enabled,
                peak_hours: self.time_aware.peak_hours.clone(),
                off_peak_factor: self.time_aware.off_peak_factor,
                preferred_auths_per_time_slot: self
                    .time_aware
                    .preferred_auths_per_time_slot
                    .clone(),
            },
            quota_aware: self.quota_aware.clone(),
            policy: self.policy.clone(),
            log: self.log.clone(),
        }
    }
}

impl TimeSlot {
    /// Validate time slot configuration
    pub fn validate(&mut self) {
        // Validate hour range (0-23)
        if self.start_hour < 0 || self.start_hour > 23 {
            self.start_hour = 0;
        }
        if self.end_hour < 0 || self.end_hour > 23 {
            self.end_hour = 23;
        }
        // Ensure start_hour <= end_hour
        if self.start_hour > self.end_hour {
            std::mem::swap(&mut self.start_hour, &mut self.end_hour);
        }

        // Validate days of week (0-6)
        self.days_of_week.retain(|day| (0..=6).contains(day));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SmartRoutingConfig::default();
        assert!(config.enabled);
        assert_eq!(config.strategy, "weighted");
        assert!(config.weight.success_rate_weight > 0.0);
    }

    #[test]
    fn test_config_normalization() {
        let mut config = SmartRoutingConfig::default();
        config.weight.success_rate_weight = 2.0;
        config.weight.latency_weight = 2.0;
        config.normalize();

        // Weights should sum to approximately 1.0
        let sum = config.weight.success_rate_weight
            + config.weight.latency_weight
            + config.weight.health_weight
            + config.weight.load_weight
            + config.weight.priority_weight;
        assert!((sum - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_config_validation() {
        let config = SmartRoutingConfig {
            strategy: "invalid".to_string(),
            ..Default::default()
        };
        let mut config = config;
        config.validate().unwrap();
        assert_eq!(config.strategy, "weighted");
    }

    #[test]
    fn test_time_slot_validation() {
        let mut slot = TimeSlot {
            start_hour: 25,
            end_hour: -5,
            days_of_week: vec![0, 1, 8],
            factor: 1.0,
        };
        slot.validate();
        assert!(slot.start_hour <= 23);
        assert!(slot.end_hour >= 0);
        assert!(!slot.days_of_week.contains(&8));
    }

    #[test]
    fn test_clone_config() {
        let config = SmartRoutingConfig::default();
        let cloned = config.clone_config();
        assert_eq!(config.strategy, cloned.strategy);
        assert_eq!(config.enabled, cloned.enabled);
    }

    #[test]
    fn test_weight_config_default() {
        let weight = WeightConfig::default();
        assert!(weight.success_rate_weight > 0.0);
        assert!(weight.unhealthy_penalty > 0.0);
        assert!(weight.unhealthy_penalty < 1.0);
    }

    #[test]
    fn test_health_config_default() {
        let health = HealthConfig::default();
        assert!(health.healthy_threshold > 0);
        assert!(health.unhealthy_threshold > 0);
        assert!(health.degraded_threshold > 0.0 && health.degraded_threshold < 1.0);
    }

    // ========================================
    // TimeSlot Validation Tests
    // ========================================

    #[test]
    fn test_time_slot_validation_valid() {
        let mut slot = TimeSlot {
            start_hour: 9,
            end_hour: 17,
            days_of_week: vec![1, 2, 3, 4, 5], // Mon-Fri
            factor: 1.5,
        };
        slot.validate();
        assert_eq!(slot.start_hour, 9);
        assert_eq!(slot.end_hour, 17);
        assert_eq!(slot.days_of_week, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_time_slot_validation_start_hour_too_high() {
        let mut slot = TimeSlot {
            start_hour: 25,
            end_hour: 17,
            days_of_week: vec![],
            factor: 1.0,
        };
        slot.validate();
        assert_eq!(slot.start_hour, 0, "Start hour > 23 should be reset to 0");
    }

    #[test]
    fn test_time_slot_validation_start_hour_negative() {
        let mut slot = TimeSlot {
            start_hour: -5,
            end_hour: 17,
            days_of_week: vec![],
            factor: 1.0,
        };
        slot.validate();
        assert_eq!(
            slot.start_hour, 0,
            "Negative start hour should be reset to 0"
        );
    }

    #[test]
    fn test_time_slot_validation_end_hour_too_high() {
        let mut slot = TimeSlot {
            start_hour: 9,
            end_hour: 30,
            days_of_week: vec![],
            factor: 1.0,
        };
        slot.validate();
        assert_eq!(slot.end_hour, 23, "End hour > 23 should be reset to 23");
    }

    #[test]
    fn test_time_slot_validation_end_hour_negative() {
        let mut slot = TimeSlot {
            start_hour: 9,
            end_hour: -1,
            days_of_week: vec![],
            factor: 1.0,
        };
        slot.validate();
        assert_eq!(slot.end_hour, 23, "Negative end hour should be reset to 23");
    }

    #[test]
    fn test_time_slot_validation_boundary_values() {
        // Test boundary: 0 and 23
        let mut slot = TimeSlot {
            start_hour: 0,
            end_hour: 23,
            days_of_week: vec![],
            factor: 1.0,
        };
        slot.validate();
        assert_eq!(slot.start_hour, 0);
        assert_eq!(slot.end_hour, 23);
    }

    #[test]
    fn test_time_slot_validation_swap_order() {
        // If start > end, they should be swapped
        let mut slot = TimeSlot {
            start_hour: 20,
            end_hour: 8,
            days_of_week: vec![],
            factor: 1.0,
        };
        slot.validate();
        assert_eq!(slot.start_hour, 8, "Start should be swapped to lower value");
        assert_eq!(slot.end_hour, 20, "End should be swapped to higher value");
    }

    #[test]
    fn test_time_slot_validation_days_of_week_invalid() {
        let mut slot = TimeSlot {
            start_hour: 9,
            end_hour: 17,
            days_of_week: vec![0, 7, 8, -1, 3], // 0 and 3 are valid, others invalid
            factor: 1.0,
        };
        slot.validate();
        assert!(slot.days_of_week.contains(&0), "0 (Sunday) should be kept");
        assert!(
            slot.days_of_week.contains(&3),
            "3 (Wednesday) should be kept"
        );
        assert!(
            !slot.days_of_week.contains(&7),
            "7 is invalid, should be removed"
        );
        assert!(
            !slot.days_of_week.contains(&8),
            "8 is invalid, should be removed"
        );
        assert!(
            !slot.days_of_week.contains(&-1),
            "-1 is invalid, should be removed"
        );
    }

    #[test]
    fn test_time_slot_validation_days_of_week_all_valid() {
        let mut slot = TimeSlot {
            start_hour: 9,
            end_hour: 17,
            days_of_week: vec![0, 1, 2, 3, 4, 5, 6], // All valid
            factor: 1.0,
        };
        slot.validate();
        assert_eq!(slot.days_of_week.len(), 7);
    }

    #[test]
    fn test_time_slot_validation_days_of_week_all_invalid() {
        let mut slot = TimeSlot {
            start_hour: 9,
            end_hour: 17,
            days_of_week: vec![7, 8, 9, -1],
            factor: 1.0,
        };
        slot.validate();
        assert!(
            slot.days_of_week.is_empty(),
            "All invalid days should be removed"
        );
    }

    // ========================================
    // TimeAwareConfig Tests
    // ========================================

    #[test]
    fn test_time_aware_config_default() {
        let config = TimeAwareConfig::default();
        assert!(!config.enabled, "Time-aware should be disabled by default");
        assert!(config.peak_hours.is_empty());
        assert!((config.off_peak_factor - 1.2).abs() < 0.01);
    }

    #[test]
    fn test_time_aware_config_with_peak_hours() {
        let config = TimeAwareConfig {
            enabled: true,
            peak_hours: vec![
                TimeSlot {
                    start_hour: 9,
                    end_hour: 12,
                    days_of_week: vec![1, 2, 3, 4, 5],
                    factor: 1.5,
                },
                TimeSlot {
                    start_hour: 14,
                    end_hour: 17,
                    days_of_week: vec![1, 2, 3, 4, 5],
                    factor: 1.3,
                },
            ],
            off_peak_factor: 0.8,
            preferred_auths_per_time_slot: HashMap::new(),
        };
        assert!(config.enabled);
        assert_eq!(config.peak_hours.len(), 2);
    }

    #[test]
    fn test_time_aware_config_off_peak_factor_validation() {
        let mut config = SmartRoutingConfig {
            time_aware: TimeAwareConfig {
                off_peak_factor: -1.0, // Invalid
                ..Default::default()
            },
            ..Default::default()
        };
        config.validate().unwrap();
        assert!(
            (config.time_aware.off_peak_factor - 1.2).abs() < 0.01,
            "Invalid off_peak_factor should be reset to default 1.2"
        );
    }

    #[test]
    fn test_time_aware_config_peak_hours_validation() {
        let mut config = SmartRoutingConfig {
            time_aware: TimeAwareConfig {
                enabled: true,
                peak_hours: vec![TimeSlot {
                    start_hour: 25,        // Invalid
                    end_hour: -5,          // Invalid
                    days_of_week: vec![8], // Invalid
                    factor: 1.0,
                }],
                ..Default::default()
            },
            ..Default::default()
        };
        config.validate().unwrap();
        let slot = &config.time_aware.peak_hours[0];
        assert_eq!(slot.start_hour, 0);
        assert_eq!(slot.end_hour, 23);
        assert!(slot.days_of_week.is_empty());
    }

    // ========================================
    // QuotaAwareConfig Tests
    // ========================================

    #[test]
    fn test_quota_aware_config_default() {
        let config = QuotaAwareConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.quota_balance_strategy, "adaptive");
        assert!((config.reserve_ratio - 0.2).abs() < 0.01);
        assert_eq!(config.recovery_window_seconds, 3600);
    }

    #[test]
    fn test_quota_aware_config_least_used_strategy() {
        let mut config = SmartRoutingConfig {
            quota_aware: QuotaAwareConfig {
                enabled: true,
                quota_balance_strategy: "least_used".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        config.validate().unwrap();
        assert_eq!(config.quota_aware.quota_balance_strategy, "least_used");
    }

    #[test]
    fn test_quota_aware_config_round_robin_strategy() {
        let mut config = SmartRoutingConfig {
            quota_aware: QuotaAwareConfig {
                enabled: true,
                quota_balance_strategy: "round_robin".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        config.validate().unwrap();
        assert_eq!(config.quota_aware.quota_balance_strategy, "round_robin");
    }

    #[test]
    fn test_quota_aware_config_adaptive_strategy() {
        let mut config = SmartRoutingConfig {
            quota_aware: QuotaAwareConfig {
                enabled: true,
                quota_balance_strategy: "adaptive".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        config.validate().unwrap();
        assert_eq!(config.quota_aware.quota_balance_strategy, "adaptive");
    }

    #[test]
    fn test_quota_aware_config_invalid_strategy() {
        let mut config = SmartRoutingConfig {
            quota_aware: QuotaAwareConfig {
                enabled: true,
                quota_balance_strategy: "invalid_strategy".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        config.validate().unwrap();
        assert_eq!(
            config.quota_aware.quota_balance_strategy, "adaptive",
            "Invalid strategy should reset to adaptive"
        );
    }

    #[test]
    fn test_quota_aware_config_reserve_ratio_negative() {
        let mut config = SmartRoutingConfig {
            quota_aware: QuotaAwareConfig {
                reserve_ratio: -0.5,
                ..Default::default()
            },
            ..Default::default()
        };
        config.normalize();
        assert!(
            (config.quota_aware.reserve_ratio - 0.2).abs() < 0.01,
            "Negative reserve_ratio should be reset to 0.2"
        );
    }

    #[test]
    fn test_quota_aware_config_reserve_ratio_over_one() {
        let mut config = SmartRoutingConfig {
            quota_aware: QuotaAwareConfig {
                reserve_ratio: 1.5,
                ..Default::default()
            },
            ..Default::default()
        };
        config.normalize();
        assert!(
            (config.quota_aware.reserve_ratio - 0.2).abs() < 0.01,
            "reserve_ratio > 1.0 should be reset to 0.2"
        );
    }

    #[test]
    fn test_quota_aware_config_reserve_ratio_valid() {
        let mut config = SmartRoutingConfig {
            quota_aware: QuotaAwareConfig {
                reserve_ratio: 0.3,
                ..Default::default()
            },
            ..Default::default()
        };
        config.normalize();
        assert!(
            (config.quota_aware.reserve_ratio - 0.3).abs() < 0.01,
            "Valid reserve_ratio should be kept"
        );
    }

    #[test]
    fn test_quota_aware_config_recovery_window_negative() {
        let mut config = SmartRoutingConfig {
            quota_aware: QuotaAwareConfig {
                recovery_window_seconds: -100,
                ..Default::default()
            },
            ..Default::default()
        };
        config.normalize();
        assert_eq!(
            config.quota_aware.recovery_window_seconds, 3600,
            "Negative recovery_window should be reset to 3600"
        );
    }

    #[test]
    fn test_quota_aware_config_recovery_window_zero() {
        let mut config = SmartRoutingConfig {
            quota_aware: QuotaAwareConfig {
                recovery_window_seconds: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        config.normalize();
        assert_eq!(
            config.quota_aware.recovery_window_seconds, 3600,
            "Zero recovery_window should be reset to 3600"
        );
    }

    // ========================================
    // StatusCodeHealthConfig Tests
    // ========================================

    #[test]
    fn test_status_code_config_default() {
        let config = StatusCodeHealthConfig::default();
        assert!(config.healthy.contains(&200));
        assert!(config.healthy.contains(&201));
        assert!(config.degraded.contains(&429));
        assert!(config.degraded.contains(&503));
        assert!(config.unhealthy.contains(&401));
        assert!(config.unhealthy.contains(&500));
    }

    // ========================================
    // PolicyConfig Tests
    // ========================================

    #[test]
    fn test_policy_config_default() {
        let config = PolicyConfig::default();
        assert!(!config.enabled);
        assert!(config.config_path.is_none());
        assert!(config.inline_policies.is_empty());
        assert!(config.cache_enabled);
    }

    // ========================================
    // LogConfig Tests
    // ========================================

    #[test]
    fn test_log_config_default() {
        let config = LogConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.level, "info");
    }
}
