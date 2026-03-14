//! `SQLite` store read/write operations.

use super::super::error::{Result, SqliteError};
use super::{CacheEntry, SQLiteStore};
use crate::health::{AuthHealth, HealthStatus};
use crate::metrics::AuthMetrics;
use chrono::{DateTime, Utc};

impl SQLiteStore {
    /// Write metrics to database
    #[allow(clippy::significant_drop_tightening)]
    pub async fn write_metrics(&self, auth_id: &str, metrics: &AuthMetrics) -> Result<()> {
        {
            let db = self.db.lock().await;

            let last_request_time = metrics.last_request_time.to_rfc3339();
            let last_success_time = metrics.last_success_time.map(|t| t.to_rfc3339());
            let last_failure_time = metrics.last_failure_time.map(|t| t.to_rfc3339());

            db.execute(
                "INSERT INTO auth_metrics (
                    auth_id, total_requests, success_count, failure_count,
                    avg_latency_ms, min_latency_ms, max_latency_ms,
                    success_rate, error_rate,
                    consecutive_successes, consecutive_failures,
                    last_request_time, last_success_time, last_failure_time,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, datetime('now'))
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
                rusqlite::params![
                    auth_id,
                    metrics.total_requests,
                    metrics.success_count,
                    metrics.failure_count,
                    metrics.avg_latency_ms,
                    metrics.min_latency_ms,
                    metrics.max_latency_ms,
                    metrics.success_rate,
                    metrics.error_rate,
                    metrics.consecutive_successes,
                    metrics.consecutive_failures,
                    last_request_time,
                    last_success_time,
                    last_failure_time,
                ],
            )
            .map_err(|e| SqliteError::query("write_metrics", e))?;
        }

        // Update cache
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

    /// Write health to database
    #[allow(clippy::significant_drop_tightening)]
    pub async fn write_health(&self, auth_id: &str, health: &AuthHealth) -> Result<()> {
        {
            let db = self.db.lock().await;

            let status = format!("{:?}", health.status);
            let last_status_change = health.last_status_change.to_rfc3339();
            let last_check_time = health.last_check_time.to_rfc3339();
            let unavailable_until = health.unavailable_until.map(|t| t.to_rfc3339());
            let error_counts =
                serde_json::to_string(&health.error_counts).unwrap_or_else(|_| "{}".to_string());

            db.execute(
                "INSERT INTO auth_health (
                    auth_id, status, consecutive_successes, consecutive_failures,
                    last_status_change, last_check_time, unavailable_until,
                    error_counts, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))
                ON CONFLICT(auth_id) DO UPDATE SET
                    status = excluded.status,
                    consecutive_successes = excluded.consecutive_successes,
                    consecutive_failures = excluded.consecutive_failures,
                    last_status_change = excluded.last_status_change,
                    last_check_time = excluded.last_check_time,
                    unavailable_until = excluded.unavailable_until,
                    error_counts = excluded.error_counts,
                    updated_at = datetime('now')",
                rusqlite::params![
                    auth_id,
                    status,
                    health.consecutive_successes,
                    health.consecutive_failures,
                    last_status_change,
                    last_check_time,
                    unavailable_until,
                    error_counts,
                ],
            )
            .map_err(|e| SqliteError::query("write_health", e))?;
        }

        // Update cache
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

    /// Write status code history entry
    #[allow(clippy::significant_drop_tightening)]
    pub async fn write_status_history(
        &self,
        auth_id: &str,
        status_code: i32,
        latency_ms: f64,
        success: bool,
    ) -> Result<()> {
        {
            let db = self.db.lock().await;

            db.execute(
                "INSERT INTO status_code_history (auth_id, status_code, latency_ms, success, timestamp)
                VALUES (?1, ?2, ?3, ?4, datetime('now'))",
                rusqlite::params![auth_id, status_code, latency_ms, i32::from(success),],
            )
            .map_err(|e| SqliteError::query("write_status_history", e))?;
        }

        Ok(())
    }

    /// Load metrics from database
    #[allow(clippy::significant_drop_tightening)]
    pub async fn load_metrics(&self, auth_id: &str) -> Result<Option<AuthMetrics>> {
        // Check cache first
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

        let result = {
            let db = self.db.lock().await;

            let mut stmt = db
                .prepare(
                    "SELECT total_requests, success_count, failure_count,
                    avg_latency_ms, min_latency_ms, max_latency_ms,
                    success_rate, error_rate,
                    consecutive_successes, consecutive_failures,
                    last_request_time, last_success_time, last_failure_time
                    FROM auth_metrics WHERE auth_id = ?1",
                )
                .map_err(|e| SqliteError::query("prepare_metrics_query", e))?;

            stmt.query_row([auth_id], |row| {
                let last_request_time_str: String = row.get(10)?;
                let last_success_time_str: Option<String> = row.get(11)?;
                let last_failure_time_str: Option<String> = row.get(12)?;

                Ok(AuthMetrics {
                    total_requests: row.get(0)?,
                    success_count: row.get(1)?,
                    failure_count: row.get(2)?,
                    avg_latency_ms: row.get(3)?,
                    min_latency_ms: row.get(4)?,
                    max_latency_ms: row.get(5)?,
                    success_rate: row.get(6)?,
                    error_rate: row.get(7)?,
                    consecutive_successes: row.get(8)?,
                    consecutive_failures: row.get(9)?,
                    last_request_time: DateTime::parse_from_rfc3339(&last_request_time_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                10,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            )
                        })?,
                    last_success_time: last_success_time_str
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    last_failure_time: last_failure_time_str
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                })
            })
        };

        match result {
            Ok(metrics) => {
                // Update cache
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
            },
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(SqliteError::query("load_metrics", e)),
        }
    }

    /// Load health from database
    #[allow(clippy::significant_drop_tightening)]
    pub async fn load_health(&self, auth_id: &str) -> Result<Option<AuthHealth>> {
        // Check cache first
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

        let result = {
            let db = self.db.lock().await;

            let mut stmt = db
                .prepare(
                    "SELECT status, consecutive_successes, consecutive_failures,
                    last_status_change, last_check_time, unavailable_until, error_counts
                    FROM auth_health WHERE auth_id = ?1",
                )
                .map_err(|e| SqliteError::query("prepare_health_query", e))?;

            stmt.query_row([auth_id], |row| {
                let status_str: String = row.get(0)?;
                let last_status_change_str: String = row.get(3)?;
                let last_check_time_str: String = row.get(4)?;
                let unavailable_until_str: Option<String> = row.get(5)?;
                let error_counts_str: String = row.get(6)?;

                let status = match status_str.as_str() {
                    "Degraded" => HealthStatus::Degraded,
                    "Unhealthy" => HealthStatus::Unhealthy,
                    _ => HealthStatus::Healthy,
                };

                let error_counts: std::collections::HashMap<i32, i32> =
                    serde_json::from_str(&error_counts_str).unwrap_or_default();

                Ok(AuthHealth {
                    status,
                    consecutive_successes: row.get(1)?,
                    consecutive_failures: row.get(2)?,
                    last_status_change: DateTime::parse_from_rfc3339(&last_status_change_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                3,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            )
                        })?,
                    last_check_time: DateTime::parse_from_rfc3339(&last_check_time_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                4,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            )
                        })?,
                    unavailable_until: unavailable_until_str
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    error_counts,
                })
            })
        };

        match result {
            Ok(health) => {
                // Update cache
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
            },
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(SqliteError::query("load_health", e)),
        }
    }

    /// Load all metrics from database
    #[allow(clippy::significant_drop_tightening)]
    pub async fn load_all_metrics(&self) -> Result<std::collections::HashMap<String, AuthMetrics>> {
        let db = self.db.lock().await;

        let mut stmt = db
            .prepare(
                "SELECT auth_id, total_requests, success_count, failure_count,
                avg_latency_ms, min_latency_ms, max_latency_ms,
                success_rate, error_rate,
                consecutive_successes, consecutive_failures,
                last_request_time, last_success_time, last_failure_time
                FROM auth_metrics",
            )
            .map_err(|e| SqliteError::query("prepare_all_metrics_query", e))?;

        let mut rows = stmt
            .query([])
            .map_err(|e| SqliteError::query("query_all_metrics", e))?;

        let mut metrics_map = Vec::new();

        while let Some(row) = rows
            .next()
            .map_err(|e| SqliteError::query("read_metric_row", e))?
        {
            let auth_id: String = row.get(0)?;
            let last_request_time_str: String = row.get(11)?;
            let last_success_time_str: Option<String> = row.get(12)?;
            let last_failure_time_str: Option<String> = row.get(13)?;

            metrics_map.push((
                auth_id,
                AuthMetrics {
                    total_requests: row.get(1)?,
                    success_count: row.get(2)?,
                    failure_count: row.get(3)?,
                    avg_latency_ms: row.get(4)?,
                    min_latency_ms: row.get(5)?,
                    max_latency_ms: row.get(6)?,
                    success_rate: row.get(7)?,
                    error_rate: row.get(8)?,
                    consecutive_successes: row.get(9)?,
                    consecutive_failures: row.get(10)?,
                    last_request_time: DateTime::parse_from_rfc3339(&last_request_time_str)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                    last_success_time: last_success_time_str
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    last_failure_time: last_failure_time_str
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                },
            ));
        }

        Ok(metrics_map.into_iter().collect())
    }

    /// Load all health from database
    #[allow(clippy::significant_drop_tightening)]
    pub async fn load_all_health(&self) -> Result<std::collections::HashMap<String, AuthHealth>> {
        let db = self.db.lock().await;

        let mut stmt = db
            .prepare(
                "SELECT auth_id, status, consecutive_successes, consecutive_failures,
                last_status_change, last_check_time, unavailable_until, error_counts
                FROM auth_health",
            )
            .map_err(|e| SqliteError::query("prepare_all_health_query", e))?;

        let mut rows = stmt
            .query([])
            .map_err(|e| SqliteError::query("query_all_health", e))?;

        let mut health_map = Vec::new();

        while let Some(row) = rows
            .next()
            .map_err(|e| SqliteError::query("read_health_row", e))?
        {
            let auth_id: String = row.get(0)?;
            let status_str: String = row.get(1)?;
            let last_status_change_str: String = row.get(3)?;
            let last_check_time_str: String = row.get(4)?;
            let unavailable_until_str: Option<String> = row.get(5)?;
            let error_counts_str: String = row.get(6)?;

            let status = match status_str.as_str() {
                "Degraded" => HealthStatus::Degraded,
                "Unhealthy" => HealthStatus::Unhealthy,
                _ => HealthStatus::Healthy,
            };

            let error_counts: std::collections::HashMap<i32, i32> =
                serde_json::from_str(&error_counts_str).unwrap_or_default();

            health_map.push((
                auth_id,
                AuthHealth {
                    status,
                    consecutive_successes: row.get(2)?,
                    consecutive_failures: row.get(3)?,
                    last_status_change: DateTime::parse_from_rfc3339(&last_status_change_str)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                    last_check_time: DateTime::parse_from_rfc3339(&last_check_time_str)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                    unavailable_until: unavailable_until_str
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    error_counts,
                },
            ));
        }

        Ok(health_map.into_iter().collect())
    }

    /// Cleanup old history records
    #[allow(clippy::significant_drop_tightening)]
    pub async fn cleanup_old_history(&self, max_age_seconds: i64) -> Result<i64> {
        let cutoff = Utc::now() - chrono::Duration::seconds(max_age_seconds);
        let cutoff_str = cutoff.to_rfc3339();

        let result = {
            let db = self.db.lock().await;

            db.execute(
                "DELETE FROM status_code_history WHERE timestamp < ?1",
                [&cutoff_str],
            )
            .map_err(|e| SqliteError::query("cleanup_old_history", e))?
        };

        Ok(result as i64)
    }

    /// Get history statistics
    #[allow(clippy::significant_drop_tightening)]
    pub async fn get_history_stats(&self) -> Result<(i64, Option<DateTime<Utc>>)> {
        let db = self.db.lock().await;

        let mut stmt = db
            .prepare("SELECT COUNT(*), MIN(timestamp) FROM status_code_history")
            .map_err(|e| SqliteError::query("prepare_history_stats_query", e))?;

        stmt.query_row([], |row| {
            let count: i64 = row.get(0)?;
            let min_timestamp_str: Option<String> = row.get(1)?;

            let min_timestamp = min_timestamp_str
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            Ok((count, min_timestamp))
        })
        .map_err(|e| SqliteError::query("get_history_stats", e))
    }
}
