//! Health tracking for credential management
//!
//! Tracks health status for credentials/auths based on request outcomes.
//! Uses configurable thresholds and status code mappings to determine
//! health state transitions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

mod state_machine;

#[cfg(test)]
mod tests;

/// Health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Healthy
    Healthy,
    /// Degraded
    Degraded,
    /// Unhealthy
    Unhealthy,
}

/// Auth health details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthHealth {
    /// Current health status
    pub status: HealthStatus,
    /// Consecutive success count
    pub consecutive_successes: i32,
    /// Consecutive failure count
    pub consecutive_failures: i32,
    /// Last status change time
    pub last_status_change: DateTime<Utc>,
    /// Last check time
    pub last_check_time: DateTime<Utc>,
    /// Unavailable until (cooldown period)
    pub unavailable_until: Option<DateTime<Utc>>,
    /// Error counts by status code
    pub error_counts: HashMap<i32, i32>,
}

/// Health manager
///
/// Tracks health status for credentials/auths. Internally uses a `tokio::RwLock<HashMap>`
/// for concurrent access.
///
/// # Clone Semantics
///
/// **Clones share the same underlying health state.** `Clone::clone()` creates a new
/// handle that points to the same `Arc<RwLock<HashMap>>` — both the original and the clone
/// see the same health events. This means:
///
/// - A clone sees all existing health state from the original
/// - Updates to the clone are visible to the original, and vice versa
/// - The `op_count` counter is also shared between clones (via `Arc<AtomicI64>`)
///
/// # Example
///
/// ```ignore
/// let manager = HealthManager::new(config);
/// manager.update_from_result("cred-1", false, 500).await;
///
/// let clone = manager.clone(); // clone shares the same health map
/// assert_eq!(clone.get_status("cred-1").await, HealthStatus::Unhealthy);
/// ```
pub struct HealthManager {
    health: Arc<tokio::sync::RwLock<HashMap<String, AuthHealth>>>,
    config: crate::config::HealthConfig,
    max_entries: usize,
    cleanup_interval: i64,
    op_count: Arc<std::sync::atomic::AtomicI64>,
}

/// Clone creates a new `HealthManager` that shares the same underlying health storage.
///
/// The clone inherits the same `config`, `max_entries`, and `cleanup_interval`,
/// and points to the same health map via `Arc`. Updates through either the original
/// or the clone are visible to both. See [`HealthManager`] struct docs for details.
impl Clone for HealthManager {
    fn clone(&self) -> Self {
        Self {
            health: Arc::clone(&self.health),
            config: self.config.clone(),
            max_entries: self.max_entries,
            cleanup_interval: self.cleanup_interval,
            op_count: Arc::clone(&self.op_count),
        }
    }
}
