use super::store::SQLiteStore;
use crate::health::{AuthHealth, HealthStatus};
use crate::metrics::AuthMetrics;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// SQLite metrics collector with periodic flushing
pub struct SQLiteMetricsCollector {
    store: SQLiteStore,
    cache: Arc<RwLock<HashMap<String, AuthMetrics>>>,
    dirty: Arc<RwLock<HashMap<String, bool>>>,
    flush_interval: Duration,
}

impl SQLiteMetricsCollector {
    /// Create a new SQLite metrics collector
    pub fn new(store: SQLiteStore) -> Self {
        Self {
            store,
            cache: Arc::new(RwLock::new(HashMap::new())),
            dirty: Arc::new(RwLock::new(HashMap::new())),
            flush_interval: Duration::from_secs(1),
        }
    }

    /// Create with custom flush interval
    pub fn with_flush_interval(store: SQLiteStore, flush_interval: Duration) -> Self {
        Self {
            store,
            cache: Arc::new(RwLock::new(HashMap::new())),
            dirty: Arc::new(RwLock::new(HashMap::new())),
            flush_interval,
        }
    }

    /// Initialize auth metrics
    pub async fn initialize_auth(&self, auth_id: &str) {
        if auth_id.is_empty() {
            return;
        }

        let mut cache = self.cache.write().await;
        cache.insert(
            auth_id.to_string(),
            AuthMetrics {
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
            },
        );
    }

    /// Record request result
    pub async fn record_request(
        &self,
        auth_id: &str,
        latency_ms: f64,
        success: bool,
        status_code: i32,
    ) {
        if auth_id.is_empty() {
            return;
        }

        // Update cache
        let mut cache = self.cache.write().await;
        let entry = cache
            .entry(auth_id.to_string())
            .or_insert_with(|| AuthMetrics {
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
            });

        entry.total_requests += 1;
        entry.last_request_time = chrono::Utc::now();

        if success {
            entry.success_count += 1;
            entry.consecutive_successes += 1;
            entry.consecutive_failures = 0;
            entry.last_success_time = Some(chrono::Utc::now());
        } else {
            entry.failure_count += 1;
            entry.consecutive_failures += 1;
            entry.consecutive_successes = 0;
            entry.last_failure_time = Some(chrono::Utc::now());
        }

        // Update latency
        if entry.min_latency_ms == 0.0 || latency_ms < entry.min_latency_ms {
            entry.min_latency_ms = latency_ms;
        }
        if latency_ms > entry.max_latency_ms {
            entry.max_latency_ms = latency_ms;
        }

        // EWMA for average latency
        let alpha = 0.1;
        entry.avg_latency_ms = alpha * latency_ms + (1.0 - alpha) * entry.avg_latency_ms;

        // Update success rate
        if entry.total_requests > 0 {
            entry.success_rate = entry.success_count as f64 / entry.total_requests as f64;
            entry.error_rate = entry.failure_count as f64 / entry.total_requests as f64;
        }

        drop(cache);

        // Mark as dirty
        let mut dirty = self.dirty.write().await;
        dirty.insert(auth_id.to_string(), true);
        drop(dirty);

        // Write status code history directly
        let _ = self
            .store
            .write_status_history(auth_id, status_code, latency_ms, success)
            .await;
    }

    /// Get metrics for auth
    pub async fn get_metrics(&self, auth_id: &str) -> Option<AuthMetrics> {
        let cache = self.cache.read().await;
        cache.get(auth_id).cloned()
    }

    /// Get all metrics
    pub async fn get_all_metrics(&self) -> HashMap<String, AuthMetrics> {
        let cache = self.cache.read().await;
        cache.clone()
    }

    /// Flush dirty data to database
    pub async fn flush(&self) -> Result<(), String> {
        // Collect dirty auth IDs
        let to_flush = {
            let dirty = self.dirty.read().await;
            dirty
                .iter()
                .filter(|&(_, &is_dirty)| is_dirty)
                .map(|(auth_id, _)| auth_id.clone())
                .collect::<Vec<_>>()
        };

        if to_flush.is_empty() {
            return Ok(());
        }

        // Flush each dirty entry
        for auth_id in to_flush {
            let cache = self.cache.read().await;
            if let Some(metrics) = cache.get(&auth_id) {
                if let Err(e) = self.store.write_metrics(&auth_id, metrics).await {
                    eprintln!("Failed to flush metrics for {}: {}", auth_id, e);
                } else {
                    // Mark as clean
                    let mut dirty = self.dirty.write().await;
                    dirty.insert(auth_id, false);
                }
            }
        }

        Ok(())
    }

    /// Load metrics from database into cache
    pub async fn load_from_db(&self) -> Result<(), String> {
        let all_metrics = self.store.load_all_metrics().await?;

        let mut cache = self.cache.write().await;
        for (auth_id, metrics) in all_metrics {
            cache.insert(auth_id, metrics);
        }

        Ok(())
    }

    /// Start periodic flush task
    pub fn start_flush_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut timer = interval(self.flush_interval);
            loop {
                timer.tick().await;
                if let Err(e) = self.flush().await {
                    eprintln!("Flush error: {}", e);
                }
            }
        })
    }
}

/// SQLite health manager with periodic flushing
pub struct SQLiteHealthManager {
    store: SQLiteStore,
    cache: Arc<RwLock<HashMap<String, AuthHealth>>>,
    dirty: Arc<RwLock<HashMap<String, bool>>>,
    flush_interval: Duration,
}

impl SQLiteHealthManager {
    /// Create a new SQLite health manager
    pub fn new(store: SQLiteStore) -> Self {
        Self {
            store,
            cache: Arc::new(RwLock::new(HashMap::new())),
            dirty: Arc::new(RwLock::new(HashMap::new())),
            flush_interval: Duration::from_secs(1),
        }
    }

    /// Create with custom flush interval
    pub fn with_flush_interval(store: SQLiteStore, flush_interval: Duration) -> Self {
        Self {
            store,
            cache: Arc::new(RwLock::new(HashMap::new())),
            dirty: Arc::new(RwLock::new(HashMap::new())),
            flush_interval,
        }
    }

    /// Record success
    pub async fn record_success(&self, auth_id: &str) {
        if auth_id.is_empty() {
            return;
        }

        let mut cache = self.cache.write().await;
        let entry = cache
            .entry(auth_id.to_string())
            .or_insert_with(|| AuthHealth {
                status: HealthStatus::Healthy,
                consecutive_successes: 0,
                consecutive_failures: 0,
                last_status_change: chrono::Utc::now(),
                last_check_time: chrono::Utc::now(),
                unavailable_until: None,
                error_counts: HashMap::new(),
            });

        entry.consecutive_successes += 1;
        entry.consecutive_failures = 0;
        entry.last_check_time = chrono::Utc::now();

        // Recover if consecutive successes reach threshold
        if entry.consecutive_successes >= 3 {
            if entry.status != HealthStatus::Healthy {
                entry.status = HealthStatus::Healthy;
                entry.last_status_change = chrono::Utc::now();
            }
            entry.unavailable_until = None;
        }

        drop(cache);

        // Mark as dirty
        let mut dirty = self.dirty.write().await;
        dirty.insert(auth_id.to_string(), true);
    }

    /// Record failure
    pub async fn record_failure(&self, auth_id: &str, status_code: i32) {
        if auth_id.is_empty() {
            return;
        }

        let mut cache = self.cache.write().await;
        let entry = cache
            .entry(auth_id.to_string())
            .or_insert_with(|| AuthHealth {
                status: HealthStatus::Healthy,
                consecutive_successes: 0,
                consecutive_failures: 0,
                last_status_change: chrono::Utc::now(),
                last_check_time: chrono::Utc::now(),
                unavailable_until: None,
                error_counts: HashMap::new(),
            });

        entry.consecutive_failures += 1;
        entry.consecutive_successes = 0;
        entry.last_check_time = chrono::Utc::now();

        // Record error count
        *entry.error_counts.entry(status_code).or_insert(0) += 1;

        // Update health status based on status code
        match status_code {
            500..=599 | 401 | 403 => {
                // Server error or auth error
                if entry.consecutive_failures >= 3 {
                    entry.status = HealthStatus::Unhealthy;
                    entry.last_status_change = chrono::Utc::now();
                    entry.unavailable_until =
                        Some(chrono::Utc::now() + chrono::Duration::seconds(60));
                }
            },
            429 => {
                // Rate limit - degraded
                entry.status = HealthStatus::Degraded;
                entry.last_status_change = chrono::Utc::now();
            },
            _ => {
                if entry.consecutive_failures >= 5 {
                    entry.status = HealthStatus::Unhealthy;
                    entry.last_status_change = chrono::Utc::now();
                    entry.unavailable_until =
                        Some(chrono::Utc::now() + chrono::Duration::seconds(60));
                }
            },
        }

        drop(cache);

        // Mark as dirty
        let mut dirty = self.dirty.write().await;
        dirty.insert(auth_id.to_string(), true);
    }

    /// Get health status
    pub async fn get_status(&self, auth_id: &str) -> HealthStatus {
        if auth_id.is_empty() {
            return HealthStatus::Healthy;
        }

        let cache = self.cache.read().await;
        cache
            .get(auth_id)
            .map(|h| h.status)
            .unwrap_or(HealthStatus::Healthy)
    }

    /// Get health details
    pub async fn get_health(&self, auth_id: &str) -> Option<AuthHealth> {
        if auth_id.is_empty() {
            return None;
        }

        let cache = self.cache.read().await;
        cache.get(auth_id).cloned()
    }

    /// Check if auth is available
    pub async fn is_available(&self, auth_id: &str) -> bool {
        if auth_id.is_empty() {
            return true;
        }

        let cache = self.cache.read().await;
        if let Some(health) = cache.get(auth_id) {
            if health.status == HealthStatus::Unhealthy {
                return false;
            }
            if let Some(unavailable_until) = health.unavailable_until {
                if chrono::Utc::now() < unavailable_until {
                    return false;
                }
            }
            return true;
        }
        true
    }

    /// Mark auth as unavailable
    pub async fn mark_unavailable(&self, auth_id: &str, duration: Duration) {
        if auth_id.is_empty() {
            return;
        }

        let mut cache = self.cache.write().await;
        let entry = cache
            .entry(auth_id.to_string())
            .or_insert_with(|| AuthHealth {
                status: HealthStatus::Healthy,
                consecutive_successes: 0,
                consecutive_failures: 0,
                last_status_change: chrono::Utc::now(),
                last_check_time: chrono::Utc::now(),
                unavailable_until: None,
                error_counts: HashMap::new(),
            });

        entry.status = HealthStatus::Unhealthy;
        entry.last_status_change = chrono::Utc::now();
        entry.unavailable_until =
            Some(chrono::Utc::now() + chrono::Duration::seconds(duration.as_secs() as i64));

        drop(cache);

        // Mark as dirty
        let mut dirty = self.dirty.write().await;
        dirty.insert(auth_id.to_string(), true);
    }

    /// Flush dirty data to database
    pub async fn flush(&self) -> Result<(), String> {
        // Collect dirty auth IDs
        let to_flush = {
            let dirty = self.dirty.read().await;
            dirty
                .iter()
                .filter(|&(_, &is_dirty)| is_dirty)
                .map(|(auth_id, _)| auth_id.clone())
                .collect::<Vec<_>>()
        };

        if to_flush.is_empty() {
            return Ok(());
        }

        // Flush each dirty entry
        for auth_id in to_flush {
            let cache = self.cache.read().await;
            if let Some(health) = cache.get(&auth_id) {
                if let Err(e) = self.store.write_health(&auth_id, health).await {
                    eprintln!("Failed to flush health for {}: {}", auth_id, e);
                } else {
                    // Mark as clean
                    let mut dirty = self.dirty.write().await;
                    dirty.insert(auth_id, false);
                }
            }
        }

        Ok(())
    }

    /// Load health from database into cache
    pub async fn load_from_db(&self) -> Result<(), String> {
        let all_health = self.store.load_all_health().await?;

        let mut cache = self.cache.write().await;
        for (auth_id, health) in all_health {
            cache.insert(auth_id, health);
        }

        Ok(())
    }

    /// Start periodic flush task
    pub fn start_flush_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut timer = interval(self.flush_interval);
            loop {
                timer.tick().await;
                if let Err(e) = self.flush().await {
                    eprintln!("Flush error: {}", e);
                }
            }
        })
    }
}
