//! Time-aware, quota-aware, and health configuration types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
        self.days_of_week.retain(|day| *day >= 0 && *day <= 6);
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
