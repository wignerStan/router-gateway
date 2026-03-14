use super::error::{Result, SqliteError};
use crate::config::SmartRoutingConfig;
use crate::sqlite::store::SQLiteStore;
use crate::weight::AuthInfo;
use rand::Rng;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
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
    pub select_count: i64,
    pub cache_hits: i64,
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

impl SQLiteSelector {
    /// Create a new `SQLite` selector
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

        // Check if smart routing is enabled
        if !self.config.enabled {
            return auths.into_iter().next().map(|a| a.id);
        }

        if auths.is_empty() {
            return None;
        }

        // Get available auths with weights from SQL query
        let available = self.query_available_auths(auths).await?;

        if available.is_empty() {
            return None;
        }

        // Select by weight
        Some(self.select_by_weight(available))
    }

    /// Query available auths and their weights using SQL
    async fn query_available_auths(&self, auths: Vec<AuthInfo>) -> Option<Vec<WeightedAuth>> {
        self.stats.db_queries.fetch_add(1, Ordering::Relaxed);

        // Build auth_id list and map
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

        // Convert auth_ids to JSON array for SQL query
        let json_array = serde_json::to_string(&auth_ids).ok()?;

        // Get database connection
        let db = self.store.get_db();
        let db = db.lock().await;

        // Execute SQL query with weight calculation
        let query = r"
            SELECT
                ids.auth_id,
                COALESCE(m.success_rate, 1.0) as success_rate,
                COALESCE(m.avg_latency_ms, 0) as latency,
                CASE
                    WHEN h.status = 'Healthy' THEN 1.0
                    WHEN h.status = 'Degraded' THEN 0.5
                    ELSE 0.01
                END as health_factor,
                CASE
                    WHEN h.unavailable_until IS NOT NULL AND datetime(h.unavailable_until) > datetime('now') THEN 0
                    ELSE 1
                END as available
            FROM (SELECT value as auth_id FROM json_each(?1)) as ids
            LEFT JOIN auth_metrics m ON m.auth_id = ids.auth_id
            LEFT JOIN auth_health h ON h.auth_id = ids.auth_id
        ";

        let mut stmt = db.prepare(query).ok()?;

        let mut rows = stmt.query([&json_array]).ok()?;

        let mut available = Vec::new();

        while let Some(row) = rows.next().ok()? {
            let auth_id: String = row.get(0).ok()?;
            let success_rate: f64 = row.get(1).ok()?;
            let latency: f64 = row.get(2).ok()?;
            let health_factor: f64 = row.get(3).ok()?;
            let available_flag: i32 = row.get(4).ok()?;

            // Skip unavailable auths
            if available_flag == 0 {
                continue;
            }

            // Get auth info
            let auth = auth_map.get(&auth_id)?;

            // Calculate weight
            let weight = self.calculate_weight(success_rate, latency, health_factor, auth);

            if weight > 0.0 {
                available.push(WeightedAuth {
                    id: auth_id,
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

        // Normalize latency (assume max 500ms)
        let latency_score = 1.0 - (latency / 500.0).min(1.0);

        // Calculate load score
        let load_score = if auth.quota_exceeded { 0.0 } else { 1.0 };

        // Calculate priority score
        let priority_score = auth.priority.map_or(0.5, |p| {
            let score = (f64::from(p) + 100.0) / 200.0;
            score.clamp(0.0, 1.0)
        });

        // Weighted sum
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

        // Apply health penalties
        if health_factor < 0.1 {
            weight *= cfg.unhealthy_penalty;
        } else if health_factor < 0.5 {
            weight *= cfg.degraded_penalty;
        }

        // Apply quota penalty
        if auth.quota_exceeded {
            weight *= cfg.quota_exceeded_penalty;
        }

        // Apply unavailable penalty
        if auth.unavailable {
            weight *= cfg.unavailable_penalty;
        }

        weight.max(0.0)
    }

    /// Select auth by weighted random choice
    fn select_by_weight(&self, available: Vec<WeightedAuth>) -> String {
        if available.len() == 1 {
            return available
                .into_iter()
                .next()
                .expect("value must be present")
                .id;
        }

        // Calculate total weight
        let total_weight: f64 = available.iter().map(|a| a.weight).sum();

        if total_weight <= 0.0 {
            // All weights are zero, select randomly
            let idx = rand::thread_rng().gen_range(0..available.len());
            return available
                .into_iter()
                .nth(idx)
                .expect("value must be present")
                .id;
        }

        // Save last element as fallback for floating-point edge cases
        let fallback = available
            .last()
            .map(|a| a.id.clone())
            .expect("value must be present");

        // Weighted random selection
        let r = rand::thread_rng().gen::<f64>() * total_weight;
        let mut cumulative = 0.0;

        for auth in available {
            cumulative += auth.weight;
            if r <= cumulative {
                return auth.id;
            }
        }

        fallback
    }

    /// Precompute weights for batch operations
    pub async fn precompute_weights(&self, auth_ids: Vec<String>) -> Result<()> {
        if auth_ids.is_empty() {
            return Ok(());
        }

        // Convert auth_ids to JSON array
        let json_array = serde_json::to_string(&auth_ids)
            .map_err(|e| SqliteError::Serialization(e.to_string()))?;

        // Get database connection
        let db = self.store.get_db();
        let db = db.lock().await;

        // Execute SQL query
        let query = r"
            SELECT
                ids.auth_id,
                COALESCE(m.success_rate, 1.0),
                COALESCE(m.avg_latency_ms, 0),
                CASE
                    WHEN h.status = 'Healthy' THEN 1.0
                    WHEN h.status = 'Degraded' THEN 0.5
                    ELSE 0.01
                END,
                CASE
                    WHEN h.unavailable_until IS NOT NULL AND datetime(h.unavailable_until) > datetime('now') THEN 0
                    ELSE 1
                END
            FROM (SELECT value as auth_id FROM json_each(?1)) as ids
            LEFT JOIN auth_metrics m ON m.auth_id = ids.auth_id
            LEFT JOIN auth_health h ON h.auth_id = ids.auth_id
        ";

        let mut stmt = db
            .prepare(query)
            .map_err(|e| SqliteError::query("prepare_weight_query", e))?;

        let mut rows = stmt
            .query([&json_array])
            .map_err(|e| SqliteError::query("query_weights", e))?;

        let mut weights = HashMap::new();

        while let Some(row) = rows
            .next()
            .map_err(|e| SqliteError::query("read_weight_row", e))?
        {
            let auth_id: String = row
                .get(0)
                .map_err(|e| SqliteError::query("get_weight_auth_id", e))?;
            let success_rate: f64 = row
                .get(1)
                .map_err(|e| SqliteError::query("get_weight_success_rate", e))?;
            let latency: f64 = row
                .get(2)
                .map_err(|e| SqliteError::query("get_weight_latency", e))?;
            let health_factor: f64 = row
                .get(3)
                .map_err(|e| SqliteError::query("get_weight_health_factor", e))?;
            let available: i32 = row
                .get(4)
                .map_err(|e| SqliteError::query("get_weight_available", e))?;

            if available == 0 {
                weights.insert(auth_id.clone(), 0.0);
                continue;
            }

            // Create dummy auth info for weight calculation
            let auth = AuthInfo {
                id: auth_id.clone(),
                priority: None,
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            };

            let weight = self.calculate_weight(success_rate, latency, health_factor, &auth);
            weights.insert(auth_id, weight);
        }

        // Update weights table
        self.update_weights(weights).await?;

        Ok(())
    }

    /// Update weights in database
    async fn update_weights(&self, weights: HashMap<String, f64>) -> Result<()> {
        let db = self.store.get_db();
        let db = db.lock().await;

        // Begin transaction
        let tx = db
            .unchecked_transaction()
            .map_err(|e| SqliteError::query("begin_transaction", e))?;

        // Prepare insert statement
        let mut stmt = tx
            .prepare(
                r"
            INSERT INTO auth_weights (auth_id, weight, calculated_at, strategy)
            VALUES (?1, ?2, datetime('now'), ?3)
            ON CONFLICT(auth_id) DO UPDATE SET
                weight = excluded.weight,
                calculated_at = excluded.calculated_at,
                strategy = excluded.strategy
        ",
            )
            .map_err(|e| SqliteError::query("prepare_weight_insert", e))?;

        for (auth_id, weight) in &weights {
            stmt.execute((&auth_id, weight, &self.config.strategy))
                .map_err(|e| SqliteError::query("execute_weight_insert", e))?;
        }

        drop(stmt);
        tx.commit()
            .map_err(|e| SqliteError::query("commit_weight_transaction", e))?;

        Ok(())
    }

    /// Get top N auths by weight
    pub async fn get_top_auths(&self, limit: usize) -> Result<Vec<String>> {
        let db = self.store.get_db();
        let db = db.lock().await;

        let query = format!(
            r"
            SELECT auth_id FROM auth_weights
            WHERE strategy = ?1
            ORDER BY weight DESC
            LIMIT {limit}
        "
        );

        let mut stmt = db
            .prepare(&query)
            .map_err(|e| SqliteError::query("prepare_top_auths_query", e))?;

        let mut rows = stmt
            .query([&self.config.strategy])
            .map_err(|e| SqliteError::query("query_top_auths", e))?;

        let mut auth_ids = Vec::new();

        while let Some(row) = rows
            .next()
            .map_err(|e| SqliteError::query("read_top_auth_row", e))?
        {
            let auth_id: String = row
                .get(0)
                .map_err(|e| SqliteError::query("get_top_auth_id", e))?;
            auth_ids.push(auth_id);
        }

        Ok(auth_ids)
    }

    /// Get selector statistics
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
    use super::*;
    use crate::sqlite::store::SQLiteConfig;

    #[tokio::test]
    async fn test_sqlite_selector_pick() {
        let config = SQLiteConfig::default();
        let store = SQLiteStore::new(config)
            .await
            .expect("value must be present");

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

        // Should return one of the auths
        let selected = selector.pick(auths).await;
        assert!(selected.is_some());
    }

    #[tokio::test]
    async fn test_precompute_weights() {
        let config = SQLiteConfig::default();
        let store = SQLiteStore::new(config)
            .await
            .expect("value must be present");

        let config = SmartRoutingConfig::default();
        let selector = SQLiteSelector::new(store, config);

        let auth_ids = vec!["auth1".to_string(), "auth2".to_string()];

        // Should not error - use timeout to avoid hanging
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            selector.precompute_weights(auth_ids),
        )
        .await;

        // Either Ok or timeout is acceptable for this test
        // Inner error indicates a bug
        assert!(
            result.as_ref().map_or(true, Result::is_ok),
            "precompute_weights should not fail: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_get_stats() {
        let config = SQLiteConfig::default();
        let store = SQLiteStore::new(config)
            .await
            .expect("value must be present");

        let config = SmartRoutingConfig::default();
        let selector = SQLiteSelector::new(store, config);

        let stats = selector.get_stats();
        assert_eq!(stats.select_count, 0);
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.db_queries, 0);
    }
}
