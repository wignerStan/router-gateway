//! Shared test fixtures using `sqlx::test` for automatic database provisioning.

use crate::routing::health::{AuthHealth, HealthStatus};
use crate::routing::metrics::AuthMetrics;
use crate::routing::sqlite::store::SQLiteStore;
use chrono::Utc;
use std::collections::HashMap;

/// Create a store backed by a fresh migrated pool (for `sqlx::test`-based tests).
#[must_use]
pub fn store_from_pool(pool: sqlx::SqlitePool) -> SQLiteStore {
    SQLiteStore::from_pool(pool, ":memory:", true)
}

/// Create sample metrics for testing.
#[must_use]
pub fn sample_metrics(
    total: i64,
    success: i64,
    failure: i64,
    avg_latency: f64,
    success_rate: f64,
) -> AuthMetrics {
    AuthMetrics {
        total_requests: total,
        success_count: success,
        failure_count: failure,
        avg_latency_ms: avg_latency,
        min_latency_ms: avg_latency * 0.5,
        max_latency_ms: avg_latency * 2.0,
        success_rate,
        error_rate: 1.0 - success_rate,
        consecutive_successes: success as i32,
        consecutive_failures: failure as i32,
        last_request_time: Utc::now(),
        last_success_time: if success > 0 { Some(Utc::now()) } else { None },
        last_failure_time: if failure > 0 { Some(Utc::now()) } else { None },
    }
}

/// Create sample health for testing.
#[must_use]
pub fn sample_health(
    status: HealthStatus,
    consecutive_successes: i32,
    consecutive_failures: i32,
) -> AuthHealth {
    AuthHealth {
        status,
        consecutive_successes,
        consecutive_failures,
        last_status_change: Utc::now(),
        last_check_time: Utc::now(),
        unavailable_until: None,
        error_counts: HashMap::new(),
    }
}

/// Create degraded health with specific error counts.
#[must_use]
pub fn degraded_health_with_errors(error_counts: HashMap<i32, i32>) -> AuthHealth {
    AuthHealth {
        status: HealthStatus::Degraded,
        consecutive_successes: 0,
        consecutive_failures: 3,
        last_status_change: Utc::now(),
        last_check_time: Utc::now(),
        unavailable_until: None,
        error_counts,
    }
}
