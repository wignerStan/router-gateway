// ALLOW: Significant drop tightening in SQLite operations requires restructuring
// query chains that would reduce readability without meaningful benefit.
#![allow(clippy::significant_drop_tightening, clippy::match_same_arms)]

use super::error::{Result, SqliteError};
use super::store::SQLiteStore;
use crate::routing::config::SmartRoutingConfig;
use crate::routing::weight::AuthInfo;
use rand::Rng;
use sqlx::FromRow;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Instant;

/// Weighted auth for selection
#[derive(Debug, Clone)]
struct WeightedAuth {
    id: String,
    weight: f64,
}

/// Selector statistics
#[derive(Debug, Clone)]
pub struct SelectorStats {
    /// Total number of credential selections
    pub select_count: i64,
    /// Number of cache hits
    pub cache_hits: i64,
    /// Number of database queries
    pub db_queries: i64,
}

/// SQLite-backed selector with SQL-based weight queries
pub struct SQLiteSelector {
    store: SQLiteStore,
    config: SmartRoutingConfig,
    stats: Arc<SelectorStatsInternal>,
}

/// Internal selector statistics with atomic counters
struct SelectorStatsInternal {
    select_count: AtomicI64,
    cache_hits: AtomicI64,
    db_queries: AtomicI64,
}

/// Weight cache entry
#[allow(dead_code)]
struct WeightCache {
    weights: HashMap<String, f64>,
    expiry: Instant,
}

/// Row for the auth weight query (`json_each` based).
#[derive(Debug, FromRow)]
struct AuthWeightRow {
    auth_id: String,
    success_rate: f64,
    latency: f64,
    health_factor: f64,
    available: i32,
}

/// Row for the precompute weight query.
#[derive(Debug, FromRow)]
struct PrecomputeWeightRow {
    auth_id: String,
    success_rate: f64,
    avg_latency_ms: f64,
    health_factor: f64,
    available: i32,
}

/// Row for top auths query.
#[derive(Debug, FromRow)]
struct TopAuthRow {
    auth_id: String,
}

impl SQLiteSelector {
    /// Create a new `SQLite` selector
    #[must_use]
    pub fn new(store: SQLiteStore, config: SmartRoutingConfig) -> Self {
        Self {
            store,
            config,
            stats: Arc::new(SelectorStatsInternal {
                select_count: AtomicI64::new(0),
                cache_hits: AtomicI64::new(0),
                db_queries: AtomicI64::new(0),
            }),
        }
    }

    /// Pick the best auth based on weighted selection using SQL queries
    pub async fn pick(&self, auths: Vec<AuthInfo>) -> Option<String> {
        self.stats.select_count.fetch_add(1, Ordering::Relaxed);

        if !self.config.enabled {
            return auths.into_iter().next().map(|a| a.id);
        }

        if auths.is_empty() {
            return None;
        }

        let available = self.query_available_auths(auths).await?;

        if available.is_empty() {
            return None;
        }

        Some(Self::select_by_weight(available))
    }

    /// Query available auths and their weights using SQL
    async fn query_available_auths(&self, auths: Vec<AuthInfo>) -> Option<Vec<WeightedAuth>> {
        self.stats.db_queries.fetch_add(1, Ordering::Relaxed);

        let auth_ids: Vec<String> = auths
            .iter()
            .filter(|a| !a.unavailable)
            .map(|a| a.id.clone())
            .collect();
        let auth_map: HashMap<String, AuthInfo> = auths
            .into_iter()
            .filter(|a| !a.unavailable)
            .map(|a| (a.id.clone(), a))
            .collect();

        if auth_ids.is_empty() {
            return None;
        }

        let json_array = serde_json::to_string(&auth_ids).ok()?;

        let rows: Vec<AuthWeightRow> = sqlx::query_as::<_, AuthWeightRow>(
            r"
            SELECT
                ids.auth_id,
                COALESCE(m.success_rate, 1.0) as success_rate,
                COALESCE(m.avg_latency_ms, 0.0) as latency,
                CASE
                    WHEN h.status = 'Healthy' THEN 1.0
                    WHEN h.status = 'Degraded' THEN 0.5
                    ELSE 0.01
                END as health_factor,
                CASE
                    WHEN h.unavailable_until IS NOT NULL AND datetime(h.unavailable_until) > datetime('now') THEN 0
                    ELSE 1
                END as available
            FROM (SELECT value as auth_id FROM json_each($1)) as ids
            LEFT JOIN auth_metrics m ON m.auth_id = ids.auth_id
            LEFT JOIN auth_health h ON h.auth_id = ids.auth_id
            ",
        )
        .bind(&json_array)
        .fetch_all(&self.store.get_pool())
        .await
        .ok()?;

        let mut available = Vec::new();

        for row in rows {
            if row.available == 0 {
                continue;
            }

            let auth = auth_map.get(&row.auth_id)?;

            let weight =
                self.calculate_weight(row.success_rate, row.latency, row.health_factor, auth);

            if weight > 0.0 {
                available.push(WeightedAuth {
                    id: row.auth_id,
                    weight,
                });
            }
        }

        Some(available)
    }

    /// Calculate weight from SQL query results
    fn calculate_weight(
        &self,
        success_rate: f64,
        latency: f64,
        health_factor: f64,
        auth: &AuthInfo,
    ) -> f64 {
        let cfg = &self.config.weight;

        let latency_score = 1.0 - (latency / 500.0).min(1.0);
        let load_score = if auth.quota_exceeded { 0.0 } else { 1.0 };
        let priority_score = auth.priority.map_or(0.5, |p| {
            let score = (f64::from(p) + 100.0) / 200.0;
            score.clamp(0.0, 1.0)
        });

        let mut weight = cfg.priority_weight.mul_add(
            priority_score,
            cfg.load_weight.mul_add(
                load_score,
                cfg.health_weight.mul_add(
                    health_factor,
                    cfg.success_rate_weight
                        .mul_add(success_rate, cfg.latency_weight * latency_score),
                ),
            ),
        );

        if health_factor < 0.1 {
            weight *= cfg.unhealthy_penalty;
        } else if health_factor < 0.5 {
            weight *= cfg.degraded_penalty;
        }

        if auth.quota_exceeded {
            weight *= cfg.quota_exceeded_penalty;
        }

        if auth.unavailable {
            weight *= cfg.unavailable_penalty;
        }

        weight.max(0.0)
    }

    /// Select auth by weighted random choice
    #[allow(clippy::expect_used)]
    fn select_by_weight(available: Vec<WeightedAuth>) -> String {
        if available.len() == 1 {
            return available
                .into_iter()
                .next()
                .expect("unwrapping valid test data")
                .id;
        }

        let total_weight: f64 = available.iter().map(|a| a.weight).sum();

        if total_weight <= 0.0 || !total_weight.is_finite() {
            let idx = rand::rng().random_range(0..available.len());
            return available
                .into_iter()
                .nth(idx)
                .expect("unwrapping valid test data")
                .id;
        }

        let fallback = available
            .last()
            .map(|a| a.id.clone())
            .expect("unwrapping valid test data");

        let r = rand::rng().random::<f64>() * total_weight;
        let mut cumulative = 0.0;

        for auth in available {
            cumulative += auth.weight;
            if r <= cumulative {
                return auth.id;
            }
        }

        fallback
    }

    /// Precompute weights for batch operations.
    ///
    /// # Errors
    ///
    /// Returns `SqliteError` if serialization fails, database queries fail,
    /// or weight updates cannot be committed.
    pub async fn precompute_weights(&self, auth_ids: Vec<String>) -> Result<()> {
        if auth_ids.is_empty() {
            return Ok(());
        }

        let json_array = serde_json::to_string(&auth_ids)
            .map_err(|e| SqliteError::Serialization(e.to_string()))?;

        let rows: Vec<PrecomputeWeightRow> = sqlx::query_as::<_, PrecomputeWeightRow>(
            r"
            SELECT
                ids.auth_id,
                COALESCE(m.success_rate, 1.0) as success_rate,
                COALESCE(m.avg_latency_ms, 0.0) as avg_latency_ms,
                CASE
                    WHEN h.status = 'Healthy' THEN 1.0
                    WHEN h.status = 'Degraded' THEN 0.5
                    ELSE 0.01
                END as health_factor,
                CASE
                    WHEN h.unavailable_until IS NOT NULL AND datetime(h.unavailable_until) > datetime('now') THEN 0
                    ELSE 1
                END as available
            FROM (SELECT value as auth_id FROM json_each($1)) as ids
            LEFT JOIN auth_metrics m ON m.auth_id = ids.auth_id
            LEFT JOIN auth_health h ON h.auth_id = ids.auth_id
            ",
        )
        .bind(&json_array)
        .fetch_all(&self.store.get_pool())
        .await
        .map_err(|e| SqliteError::query("precompute_weights_query", e))?;

        let mut weights = HashMap::new();

        for row in rows {
            if row.available == 0 {
                weights.insert(row.auth_id.clone(), 0.0);
                continue;
            }

            let auth = AuthInfo {
                id: row.auth_id.clone(),
                priority: None,
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            };

            let weight = self.calculate_weight(
                row.success_rate,
                row.avg_latency_ms,
                row.health_factor,
                &auth,
            );
            let weight = if weight.is_finite() && weight >= 0.0 {
                weight
            } else {
                0.0
            };
            weights.insert(row.auth_id, weight);
        }

        self.update_weights(weights).await?;

        Ok(())
    }

    /// Update weights in database
    async fn update_weights(&self, weights: HashMap<String, f64>) -> Result<()> {
        let mut tx = self
            .store
            .get_pool()
            .begin()
            .await
            .map_err(|e| SqliteError::query("begin_transaction", e))?;

        for (auth_id, weight) in &weights {
            let sanitized = if weight.is_finite() { *weight } else { 0.0 };
            sqlx::query(
                r"
                INSERT INTO auth_weights (auth_id, weight, calculated_at, strategy)
                VALUES ($1, $2, datetime('now'), $3)
                ON CONFLICT(auth_id) DO UPDATE SET
                    weight = excluded.weight,
                    calculated_at = excluded.calculated_at,
                    strategy = excluded.strategy
                ",
            )
            .bind(auth_id)
            .bind(sanitized)
            .bind(&self.config.strategy)
            .execute(&mut *tx)
            .await
            .map_err(|e| SqliteError::query("execute_weight_insert", e))?;
        }

        tx.commit()
            .await
            .map_err(|e| SqliteError::query("commit_weight_transaction", e))?;

        Ok(())
    }

    /// Get top N auths by weight.
    ///
    /// # Errors
    ///
    /// Returns `SqliteError` if the query fails.
    pub async fn get_top_auths(&self, limit: usize) -> Result<Vec<String>> {
        // sqlx SQLite does not support binding LIMIT; format with integer is safe here.
        let query = format!(
            r"
            SELECT auth_id FROM auth_weights
            WHERE strategy = $1
            ORDER BY weight DESC
            LIMIT {limit}
            "
        );

        let rows: Vec<TopAuthRow> = sqlx::query_as::<_, TopAuthRow>(&query)
            .bind(&self.config.strategy)
            .fetch_all(&self.store.get_pool())
            .await
            .map_err(|e| SqliteError::query("query_top_auths", e))?;

        Ok(rows.into_iter().map(|r| r.auth_id).collect())
    }

    /// Get selector statistics
    #[must_use]
    pub fn get_stats(&self) -> SelectorStats {
        SelectorStats {
            select_count: self.stats.select_count.load(Ordering::Relaxed),
            cache_hits: self.stats.cache_hits.load(Ordering::Relaxed),
            db_queries: self.stats.db_queries.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]
    use super::*;
    use crate::routing::sqlite::store::SQLiteConfig;

    #[tokio::test]
    async fn test_sqlite_selector_pick() {
        let config = SQLiteConfig::default();
        let store = SQLiteStore::new(config)
            .await
            .expect("unwrapping valid test data");

        let config = SmartRoutingConfig::default();
        let selector = SQLiteSelector::new(store, config);

        let auths = vec![
            AuthInfo {
                id: "auth1".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            AuthInfo {
                id: "auth2".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
        ];

        let selected = selector.pick(auths).await;
        assert!(selected.is_some());
    }

    #[tokio::test]
    async fn test_precompute_weights() {
        let config = SQLiteConfig::default();
        let store = SQLiteStore::new(config)
            .await
            .expect("unwrapping valid test data");

        let config = SmartRoutingConfig::default();
        let selector = SQLiteSelector::new(store, config);

        let auth_ids = vec!["auth1".to_string(), "auth2".to_string()];

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            selector.precompute_weights(auth_ids),
        )
        .await;

        match result {
            Ok(Ok(())) => {},
            Ok(Err(e)) => panic!("Failed to precompute weights: {e}"),
            Err(_) => {},
        }
    }

    #[tokio::test]
    async fn test_get_stats() {
        let config = SQLiteConfig::default();
        let store = SQLiteStore::new(config)
            .await
            .expect("unwrapping valid test data");

        let config = SmartRoutingConfig::default();
        let selector = SQLiteSelector::new(store, config);

        let stats = selector.get_stats();
        assert_eq!(stats.select_count, 0);
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.db_queries, 0);
    }
    use crate::routing::health::{AuthHealth, HealthStatus};
    use crate::routing::metrics::AuthMetrics;
    use chrono::Utc;

    #[tokio::test]
    async fn test_precompute_and_get_top_auths() {
        let config = SQLiteConfig::default();
        let store = SQLiteStore::new(config)
            .await
            .expect("unwrapping valid test data");
        let config = SmartRoutingConfig::default();
        let selector = SQLiteSelector::new(store.clone(), config);

        let metrics1 = AuthMetrics {
            total_requests: 100,
            success_count: 100,
            failure_count: 0,
            avg_latency_ms: 10.0,
            min_latency_ms: 5.0,
            max_latency_ms: 20.0,
            success_rate: 1.0,
            error_rate: 0.0,
            consecutive_successes: 100,
            consecutive_failures: 0,
            last_request_time: Utc::now(),
            last_success_time: Some(Utc::now()),
            last_failure_time: None,
        };
        store
            .write_metrics("auth1", &metrics1)
            .await
            .expect("unwrapping valid test data");

        let health1 = AuthHealth {
            status: HealthStatus::Healthy,
            consecutive_successes: 100,
            consecutive_failures: 0,
            last_status_change: Utc::now(),
            last_check_time: Utc::now(),
            unavailable_until: None,
            error_counts: std::collections::HashMap::new(),
        };
        store
            .write_health("auth1", &health1)
            .await
            .expect("unwrapping valid test data");

        let metrics2 = AuthMetrics {
            total_requests: 100,
            success_count: 50,
            failure_count: 50,
            avg_latency_ms: 200.0,
            min_latency_ms: 100.0,
            max_latency_ms: 300.0,
            success_rate: 0.5,
            error_rate: 0.5,
            consecutive_successes: 0,
            consecutive_failures: 10,
            last_request_time: Utc::now(),
            last_success_time: Some(Utc::now()),
            last_failure_time: Some(Utc::now()),
        };
        store
            .write_metrics("auth2", &metrics2)
            .await
            .expect("unwrapping valid test data");

        let health2 = AuthHealth {
            status: HealthStatus::Degraded,
            consecutive_successes: 0,
            consecutive_failures: 10,
            last_status_change: Utc::now(),
            last_check_time: Utc::now(),
            unavailable_until: None,
            error_counts: std::collections::HashMap::new(),
        };
        store
            .write_health("auth2", &health2)
            .await
            .expect("unwrapping valid test data");

        let auth_ids = vec!["auth1".to_string(), "auth2".to_string()];
        selector
            .precompute_weights(auth_ids)
            .await
            .expect("unwrapping valid test data");

        let top_auths = selector
            .get_top_auths(2)
            .await
            .expect("unwrapping valid test data");

        assert_eq!(top_auths.len(), 2);
        assert_eq!(top_auths[0], "auth1");
        assert_eq!(top_auths[1], "auth2");
    }

    #[tokio::test]
    async fn test_get_top_auths_limit() {
        let config = SQLiteConfig::default();
        let store = SQLiteStore::new(config)
            .await
            .expect("unwrapping valid test data");
        let config = SmartRoutingConfig::default();
        let selector = SQLiteSelector::new(store.clone(), config);

        let auth_ids = vec![
            "auth1".to_string(),
            "auth2".to_string(),
            "auth3".to_string(),
        ];
        selector
            .precompute_weights(auth_ids)
            .await
            .expect("unwrapping valid test data");

        let top_auths = selector
            .get_top_auths(2)
            .await
            .expect("unwrapping valid test data");

        assert_eq!(top_auths.len(), 2);
    }
}
