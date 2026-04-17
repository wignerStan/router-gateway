//! `SQLite` store read/write operations.

// ALLOW: Mutex/RwLock poisoning is an acceptable panic -- propagates failure for
// inconsistent shared state. All `.expect()` calls below are on cache locks that
// poison when a holder panics, indicating data corruption.
#![allow(clippy::expect_used)]
#![allow(clippy::significant_drop_tightening)]

use super::super::error::{Result, SqliteError};
use super::{CacheEntry, SQLiteStore};
use crate::routing::health::{AuthHealth, HealthStatus};
use crate::routing::metrics::AuthMetrics;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use std::collections::HashMap;

/// Row type for reading `auth_metrics` columns.
#[derive(Debug, FromRow)]
struct MetricsRow {
    total_requests: i64,
    success_count: i64,
    failure_count: i64,
    avg_latency_ms: f64,
    min_latency_ms: f64,
    max_latency_ms: f64,
    success_rate: f64,
    error_rate: f64,
    consecutive_successes: i32,
    consecutive_failures: i32,
    last_request_time: String,
    last_success_time: Option<String>,
    last_failure_time: Option<String>,
}

/// Row type for reading `auth_metrics` with `auth_id` (`load_all`).
#[derive(Debug, FromRow)]
struct MetricsWithIdRow {
    auth_id: String,
    total_requests: i64,
    success_count: i64,
    failure_count: i64,
    avg_latency_ms: f64,
    min_latency_ms: f64,
    max_latency_ms: f64,
    success_rate: f64,
    error_rate: f64,
    consecutive_successes: i32,
    consecutive_failures: i32,
    last_request_time: String,
    last_success_time: Option<String>,
    last_failure_time: Option<String>,
}

/// Row type for reading `auth_health` columns.
#[derive(Debug, FromRow)]
struct HealthRow {
    status: String,
    consecutive_successes: i32,
    consecutive_failures: i32,
    last_status_change: String,
    last_check_time: String,
    unavailable_until: Option<String>,
    error_counts: String,
}

/// Row type for reading `auth_health` with `auth_id` (`load_all`).
#[derive(Debug, FromRow)]
struct HealthWithIdRow {
    auth_id: String,
    status: String,
    consecutive_successes: i32,
    consecutive_failures: i32,
    last_status_change: String,
    last_check_time: String,
    unavailable_until: Option<String>,
    error_counts: String,
}

/// Parse RFC3339 datetime string, returning `Utc::now()` on failure.
fn parse_datetime(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .inspect_err(|e| {
            tracing::warn!("Failed to parse datetime '{s}': {e}");
        })
        .unwrap_or_else(|_| Utc::now())
}

/// Parse optional RFC3339 datetime string.
fn parse_datetime_opt(s: Option<&str>) -> Option<DateTime<Utc>> {
    s.and_then(|v| {
        DateTime::parse_from_rfc3339(v)
            .map(|dt| dt.with_timezone(&Utc))
            .inspect_err(|e| {
                tracing::warn!("Failed to parse datetime '{v}': {e}");
            })
            .ok()
    })
}

/// Parse health status string.
fn parse_health_status(s: &str) -> HealthStatus {
    match s {
        "Degraded" => HealthStatus::Degraded,
        "Unhealthy" => HealthStatus::Unhealthy,
        _ => HealthStatus::Healthy,
    }
}

/// Parse `error_counts` JSON string.
fn parse_error_counts(s: &str) -> HashMap<i32, i32> {
    serde_json::from_str(s).unwrap_or_else(|e| {
        tracing::warn!("Failed to parse error_counts JSON: {e}");
        HashMap::default()
    })
}

impl SQLiteStore {
    /// Write metrics to the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the SQL query fails or the cache write lock is poisoned.
    ///
    /// # Panics
    ///
    /// Panics if the cache write lock is poisoned (indicates data corruption from a prior panic).
    pub async fn write_metrics(&self, auth_id: &str, metrics: &AuthMetrics) -> Result<()> {
        let last_request_time = metrics.last_request_time.to_rfc3339();
        let last_success_time = metrics.last_success_time.map(|t| t.to_rfc3339());
        let last_failure_time = metrics.last_failure_time.map(|t| t.to_rfc3339());

        sqlx::query(
            "INSERT INTO auth_metrics (
                auth_id, total_requests, success_count, failure_count,
                avg_latency_ms, min_latency_ms, max_latency_ms,
                success_rate, error_rate,
                consecutive_successes, consecutive_failures,
                last_request_time, last_success_time, last_failure_time,
                updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, datetime('now'))
            ON CONFLICT(auth_id) DO UPDATE SET
                total_requests = excluded.total_requests,
                success_count = excluded.success_count,
                failure_count = excluded.failure_count,
                avg_latency_ms = excluded.avg_latency_ms,
                min_latency_ms = excluded.min_latency_ms,
                max_latency_ms = excluded.max_latency_ms,
                success_rate = excluded.success_rate,
                error_rate = excluded.error_rate,
                consecutive_successes = excluded.consecutive_successes,
                consecutive_failures = excluded.consecutive_failures,
                last_request_time = excluded.last_request_time,
                last_success_time = excluded.last_success_time,
                last_failure_time = excluded.last_failure_time,
                updated_at = datetime('now')",
        )
        .bind(auth_id)
        .bind(metrics.total_requests)
        .bind(metrics.success_count)
        .bind(metrics.failure_count)
        .bind(metrics.avg_latency_ms)
        .bind(metrics.min_latency_ms)
        .bind(metrics.max_latency_ms)
        .bind(metrics.success_rate)
        .bind(metrics.error_rate)
        .bind(metrics.consecutive_successes)
        .bind(metrics.consecutive_failures)
        .bind(&last_request_time)
        .bind(&last_success_time)
        .bind(&last_failure_time)
        .execute(&self.pool)
        .await
        .map_err(|e| SqliteError::query("write_metrics", e))?;

        if self
            .cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let mut cache = self
                .cache
                .write()
                .expect("cache write lock poisoned: metrics write");
            let entry = cache
                .entry(auth_id.to_string())
                .or_insert_with(|| CacheEntry {
                    metrics: None,
                    health: None,
                    timestamp: Utc::now(),
                });
            entry.metrics = Some(metrics.clone());
            entry.timestamp = Utc::now();
        }

        Ok(())
    }

    /// Write health to the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the SQL query fails or the cache write lock is poisoned.
    ///
    /// # Panics
    ///
    /// Panics if the cache write lock is poisoned (indicates data corruption from a prior panic).
    pub async fn write_health(&self, auth_id: &str, health: &AuthHealth) -> Result<()> {
        let status = format!("{:?}", health.status);
        let last_status_change = health.last_status_change.to_rfc3339();
        let last_check_time = health.last_check_time.to_rfc3339();
        let unavailable_until = health.unavailable_until.map(|t| t.to_rfc3339());
        let error_counts =
            serde_json::to_string(&health.error_counts).unwrap_or_else(|_| "{}".to_string());

        sqlx::query(
            "INSERT INTO auth_health (
                auth_id, status, consecutive_successes, consecutive_failures,
                last_status_change, last_check_time, unavailable_until,
                error_counts, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, datetime('now'))
            ON CONFLICT(auth_id) DO UPDATE SET
                status = excluded.status,
                consecutive_successes = excluded.consecutive_successes,
                consecutive_failures = excluded.consecutive_failures,
                last_status_change = excluded.last_status_change,
                last_check_time = excluded.last_check_time,
                unavailable_until = excluded.unavailable_until,
                error_counts = excluded.error_counts,
                updated_at = datetime('now')",
        )
        .bind(auth_id)
        .bind(&status)
        .bind(health.consecutive_successes)
        .bind(health.consecutive_failures)
        .bind(&last_status_change)
        .bind(&last_check_time)
        .bind(&unavailable_until)
        .bind(&error_counts)
        .execute(&self.pool)
        .await
        .map_err(|e| SqliteError::query("write_health", e))?;

        if self
            .cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let mut cache = self
                .cache
                .write()
                .expect("cache write lock poisoned: health write");
            let entry = cache
                .entry(auth_id.to_string())
                .or_insert_with(|| CacheEntry {
                    metrics: None,
                    health: None,
                    timestamp: Utc::now(),
                });
            entry.health = Some(health.clone());
            entry.timestamp = Utc::now();
        }

        Ok(())
    }

    /// Write status code history entry.
    ///
    /// # Errors
    ///
    /// Returns an error if the SQL query fails.
    pub async fn write_status_history(
        &self,
        auth_id: &str,
        status_code: i32,
        latency_ms: f64,
        success: bool,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO status_code_history (auth_id, status_code, latency_ms, success, timestamp)
            VALUES ($1, $2, $3, $4, datetime('now'))",
        )
        .bind(auth_id)
        .bind(status_code)
        .bind(latency_ms)
        .bind(i32::from(success))
        .execute(&self.pool)
        .await
        .map_err(|e| SqliteError::query("write_status_history", e))?;

        Ok(())
    }

    /// Load metrics from the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the SQL query fails or the cache lock is poisoned.
    ///
    /// # Panics
    ///
    /// Panics if the cache write lock is poisoned (indicates data corruption).
    pub async fn load_metrics(&self, auth_id: &str) -> Result<Option<AuthMetrics>> {
        if self
            .cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let cache = self
                .cache
                .read()
                .expect("cache read lock poisoned: metrics read");
            if let Some(entry) = cache.get(auth_id) {
                if let Some(ref metrics) = entry.metrics {
                    return Ok(Some(metrics.clone()));
                }
            }
        }

        let row: Option<MetricsRow> = sqlx::query_as::<_, MetricsRow>(
            "SELECT total_requests, success_count, failure_count,
            avg_latency_ms, min_latency_ms, max_latency_ms,
            success_rate, error_rate,
            consecutive_successes, consecutive_failures,
            last_request_time, last_success_time, last_failure_time
            FROM auth_metrics WHERE auth_id = $1",
        )
        .bind(auth_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SqliteError::query("load_metrics", e))?;

        let Some(row) = row else { return Ok(None) };

        let metrics = AuthMetrics {
            total_requests: row.total_requests,
            success_count: row.success_count,
            failure_count: row.failure_count,
            avg_latency_ms: row.avg_latency_ms,
            min_latency_ms: row.min_latency_ms,
            max_latency_ms: row.max_latency_ms,
            success_rate: row.success_rate,
            error_rate: row.error_rate,
            consecutive_successes: row.consecutive_successes,
            consecutive_failures: row.consecutive_failures,
            last_request_time: parse_datetime(&row.last_request_time),
            last_success_time: parse_datetime_opt(row.last_success_time.as_deref()),
            last_failure_time: parse_datetime_opt(row.last_failure_time.as_deref()),
        };

        if self
            .cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let mut cache = self
                .cache
                .write()
                .expect("cache write lock poisoned: metrics write after load");
            let entry = cache
                .entry(auth_id.to_string())
                .or_insert_with(|| CacheEntry {
                    metrics: None,
                    health: None,
                    timestamp: Utc::now(),
                });
            entry.metrics = Some(metrics.clone());
            entry.timestamp = Utc::now();
        }

        Ok(Some(metrics))
    }

    /// Load health from the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the SQL query fails or the cache lock is poisoned.
    ///
    /// # Panics
    ///
    /// Panics if the cache write lock is poisoned (indicates data corruption).
    pub async fn load_health(&self, auth_id: &str) -> Result<Option<AuthHealth>> {
        if self
            .cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let cache = self
                .cache
                .read()
                .expect("cache read lock poisoned: health read");
            if let Some(entry) = cache.get(auth_id) {
                if let Some(ref health) = entry.health {
                    return Ok(Some(health.clone()));
                }
            }
        }

        let row: Option<HealthRow> = sqlx::query_as::<_, HealthRow>(
            "SELECT status, consecutive_successes, consecutive_failures,
            last_status_change, last_check_time, unavailable_until, error_counts
            FROM auth_health WHERE auth_id = $1",
        )
        .bind(auth_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SqliteError::query("load_health", e))?;

        let Some(row) = row else { return Ok(None) };

        let health = AuthHealth {
            status: parse_health_status(&row.status),
            consecutive_successes: row.consecutive_successes,
            consecutive_failures: row.consecutive_failures,
            last_status_change: parse_datetime(&row.last_status_change),
            last_check_time: parse_datetime(&row.last_check_time),
            unavailable_until: parse_datetime_opt(row.unavailable_until.as_deref()),
            error_counts: parse_error_counts(&row.error_counts),
        };

        if self
            .cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let mut cache = self
                .cache
                .write()
                .expect("cache write lock poisoned: health write after load");
            let entry = cache
                .entry(auth_id.to_string())
                .or_insert_with(|| CacheEntry {
                    metrics: None,
                    health: None,
                    timestamp: Utc::now(),
                });
            entry.health = Some(health.clone());
            entry.timestamp = Utc::now();
        }

        Ok(Some(health))
    }

    /// Load all metrics from the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the SQL query or row mapping fails.
    pub async fn load_all_metrics(&self) -> Result<HashMap<String, AuthMetrics>> {
        let rows: Vec<MetricsWithIdRow> = sqlx::query_as::<_, MetricsWithIdRow>(
            "SELECT auth_id, total_requests, success_count, failure_count,
            avg_latency_ms, min_latency_ms, max_latency_ms,
            success_rate, error_rate,
            consecutive_successes, consecutive_failures,
            last_request_time, last_success_time, last_failure_time
            FROM auth_metrics",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SqliteError::query("load_all_metrics", e))?;

        let metrics_map: HashMap<String, AuthMetrics> = rows
            .into_iter()
            .map(|row| {
                let auth_id = row.auth_id.clone();
                let metrics = AuthMetrics {
                    total_requests: row.total_requests,
                    success_count: row.success_count,
                    failure_count: row.failure_count,
                    avg_latency_ms: row.avg_latency_ms,
                    min_latency_ms: row.min_latency_ms,
                    max_latency_ms: row.max_latency_ms,
                    success_rate: row.success_rate,
                    error_rate: row.error_rate,
                    consecutive_successes: row.consecutive_successes,
                    consecutive_failures: row.consecutive_failures,
                    last_request_time: parse_datetime(&row.last_request_time),
                    last_success_time: parse_datetime_opt(row.last_success_time.as_deref()),
                    last_failure_time: parse_datetime_opt(row.last_failure_time.as_deref()),
                };
                (auth_id, metrics)
            })
            .collect();

        Ok(metrics_map)
    }

    /// Load all health from the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the SQL query or row mapping fails.
    pub async fn load_all_health(&self) -> Result<HashMap<String, AuthHealth>> {
        let rows: Vec<HealthWithIdRow> = sqlx::query_as::<_, HealthWithIdRow>(
            "SELECT auth_id, status, consecutive_successes, consecutive_failures,
            last_status_change, last_check_time, unavailable_until, error_counts
            FROM auth_health",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SqliteError::query("load_all_health", e))?;

        let health_map: HashMap<String, AuthHealth> = rows
            .into_iter()
            .map(|row| {
                let auth_id = row.auth_id.clone();
                let health = AuthHealth {
                    status: parse_health_status(&row.status),
                    consecutive_successes: row.consecutive_successes,
                    consecutive_failures: row.consecutive_failures,
                    last_status_change: parse_datetime(&row.last_status_change),
                    last_check_time: parse_datetime(&row.last_check_time),
                    unavailable_until: parse_datetime_opt(row.unavailable_until.as_deref()),
                    error_counts: parse_error_counts(&row.error_counts),
                };
                (auth_id, health)
            })
            .collect();

        Ok(health_map)
    }

    /// Cleanup old history records.
    ///
    /// # Errors
    ///
    /// Returns an error if the delete query fails.
    pub async fn cleanup_old_history(&self, max_age_seconds: i64) -> Result<i64> {
        let max_age_seconds = max_age_seconds.max(0);
        let cutoff = Utc::now() - chrono::Duration::seconds(max_age_seconds);
        let cutoff_str = cutoff.to_rfc3339();

        let result = sqlx::query("DELETE FROM status_code_history WHERE timestamp < $1")
            .bind(&cutoff_str)
            .execute(&self.pool)
            .await
            .map_err(|e| SqliteError::query("cleanup_old_history", e))?;

        Ok(i64::try_from(result.rows_affected()).unwrap_or(i64::MAX))
    }

    /// Get history statistics.
    ///
    /// # Errors
    ///
    /// Returns an error if the query or row mapping fails.
    pub async fn get_history_stats(&self) -> Result<(i64, Option<DateTime<Utc>>)> {
        let row: Option<(i64, Option<String>)> =
            sqlx::query_as("SELECT COUNT(*), MIN(timestamp) FROM status_code_history")
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| SqliteError::query("get_history_stats", e))?;

        let Some((count, min_ts_str)) = row else {
            return Ok((0, None));
        };

        let min_timestamp = parse_datetime_opt(min_ts_str.as_deref());

        Ok((count, min_timestamp))
    }
}
