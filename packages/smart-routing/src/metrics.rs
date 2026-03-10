use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Authentication metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthMetrics {
    /// Total request count
    pub total_requests: i64,
    /// Success request count
    pub success_count: i64,
    /// Failure request count
    pub failure_count: i64,

    /// Average latency in milliseconds (EWMA)
    pub avg_latency_ms: f64,
    /// Minimum latency
    pub min_latency_ms: f64,
    /// Maximum latency
    pub max_latency_ms: f64,

    /// Success rate (EWMA)
    pub success_rate: f64,
    /// Error rate (EWMA)
    pub error_rate: f64,

    /// Consecutive success count
    pub consecutive_successes: i32,
    /// Consecutive failure count
    pub consecutive_failures: i32,

    /// Last request time
    pub last_request_time: DateTime<Utc>,
    /// Last success time
    pub last_success_time: Option<DateTime<Utc>>,
    /// Last failure time
    pub last_failure_time: Option<DateTime<Utc>>,
}

/// EWMA alpha constant for exponential weighted moving average
/// Smaller value = stronger smoothing, slower response to new data
const EWMA_ALPHA: f64 = 0.3;

/// Metrics collector
pub struct MetricsCollector {
    metrics: tokio::sync::RwLock<std::collections::HashMap<String, AuthMetrics>>,
    max_entries: usize,
    cleanup_interval: i64,
    op_count: std::sync::Arc<std::sync::atomic::AtomicI64>,
}

impl Clone for MetricsCollector {
    fn clone(&self) -> Self {
        Self {
            metrics: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            max_entries: self.max_entries,
            cleanup_interval: self.cleanup_interval,
            op_count: std::sync::Arc::clone(&self.op_count),
        }
    }
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            metrics: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            max_entries: 10_000,
            cleanup_interval: 100,
            op_count: std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0)),
        }
    }

    /// Create a metrics collector with a limit
    pub fn with_limit(max_entries: usize) -> Self {
        Self {
            metrics: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            max_entries: if max_entries > 0 { max_entries } else { 10_000 },
            cleanup_interval: 100,
            op_count: std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0)),
        }
    }

    /// Initialize auth metrics
    pub async fn initialize_auth(&self, auth_id: &str) {
        if auth_id.is_empty() {
            return;
        }

        let mut metrics = self.metrics.write().await;
        metrics.insert(
            auth_id.to_string(),
            AuthMetrics {
                total_requests: 0,
                success_count: 0,
                failure_count: 0,
                avg_latency_ms: 0.0,
                min_latency_ms: f64::MAX,
                max_latency_ms: 0.0,
                success_rate: 1.0, // Start with optimistic default
                error_rate: 0.0,
                consecutive_successes: 0,
                consecutive_failures: 0,
                last_request_time: Utc::now(),
                last_success_time: None,
                last_failure_time: None,
            },
        );

        // Cleanup old entries periodically
        if metrics.len() > self.max_entries {
            self.cleanup_old_entries(&mut metrics).await;
        }
    }

    /// Record execution result
    pub async fn record_result(
        &self,
        auth_id: &str,
        success: bool,
        latency_ms: f64,
        _status_code: i32,
    ) {
        if auth_id.is_empty() {
            return;
        }

        // Increment operation counter and check if cleanup is needed
        let op_count = self
            .op_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if op_count % self.cleanup_interval == 0 {
            let mut metrics = self.metrics.write().await;
            if metrics.len() > self.max_entries {
                self.cleanup_old_entries(&mut metrics).await;
            }
        }

        let mut metrics = self.metrics.write().await;
        let entry = metrics
            .entry(auth_id.to_string())
            .or_insert_with(|| AuthMetrics {
                total_requests: 0,
                success_count: 0,
                failure_count: 0,
                avg_latency_ms: 0.0,
                min_latency_ms: f64::MAX,
                max_latency_ms: 0.0,
                success_rate: 1.0,
                error_rate: 0.0,
                consecutive_successes: 0,
                consecutive_failures: 0,
                last_request_time: Utc::now(),
                last_success_time: None,
                last_failure_time: None,
            });

        // Update request counts
        entry.total_requests += 1;
        entry.last_request_time = Utc::now();

        if success {
            entry.success_count += 1;
            entry.consecutive_successes += 1;
            entry.consecutive_failures = 0;
            entry.last_success_time = Some(Utc::now());
        } else {
            entry.failure_count += 1;
            entry.consecutive_failures += 1;
            entry.consecutive_successes = 0;
            entry.last_failure_time = Some(Utc::now());
        }

        // Update latency using EWMA
        if latency_ms > 0.0 {
            if entry.avg_latency_ms == 0.0 {
                entry.avg_latency_ms = latency_ms;
            } else {
                entry.avg_latency_ms =
                    EWMA_ALPHA * latency_ms + (1.0 - EWMA_ALPHA) * entry.avg_latency_ms;
            }

            if latency_ms < entry.min_latency_ms {
                entry.min_latency_ms = latency_ms;
            }
            if latency_ms > entry.max_latency_ms {
                entry.max_latency_ms = latency_ms;
            }
        }

        // Update success/error rate using EWMA
        let current_success = if success { 1.0 } else { 0.0 };
        entry.success_rate = EWMA_ALPHA * current_success + (1.0 - EWMA_ALPHA) * entry.success_rate;
        entry.error_rate = 1.0 - entry.success_rate;
    }

    /// Get auth metrics
    pub async fn get_metrics(&self, auth_id: &str) -> Option<AuthMetrics> {
        if auth_id.is_empty() {
            return None;
        }

        let metrics = self.metrics.read().await;
        metrics.get(auth_id).cloned()
    }

    /// Get all metrics
    pub async fn get_all_metrics(&self) -> std::collections::HashMap<String, AuthMetrics> {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// Reset specific auth metrics
    pub async fn reset(&self, auth_id: &str) {
        if auth_id.is_empty() {
            return;
        }

        let mut metrics = self.metrics.write().await;
        metrics.insert(
            auth_id.to_string(),
            AuthMetrics {
                total_requests: 0,
                success_count: 0,
                failure_count: 0,
                avg_latency_ms: 0.0,
                min_latency_ms: f64::MAX,
                max_latency_ms: 0.0,
                success_rate: 1.0,
                error_rate: 0.0,
                consecutive_successes: 0,
                consecutive_failures: 0,
                last_request_time: Utc::now(),
                last_success_time: None,
                last_failure_time: None,
            },
        );
    }

    /// Reset all metrics
    pub async fn reset_all(&self) {
        let mut metrics = self.metrics.write().await;
        *metrics = std::collections::HashMap::new();
    }

    /// Cleanup old entries to control memory growth
    async fn cleanup_old_entries(
        &self,
        metrics: &mut std::collections::HashMap<String, AuthMetrics>,
    ) {
        if self.max_entries == 0 {
            return;
        }

        // Collect entries with last request time
        let mut entries: Vec<(String, DateTime<Utc>)> = metrics
            .iter()
            .map(|(id, m)| (id.clone(), m.last_request_time))
            .collect();

        // Sort by last request time (oldest first)
        entries.sort_by(|a, b| a.1.cmp(&b.1));

        // Remove oldest entries if over limit
        let remove_count = entries.len().saturating_sub(self.max_entries);
        for (id, _) in entries.into_iter().take(remove_count) {
            metrics.remove(&id);
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collection() {
        let collector = MetricsCollector::new();
        collector.initialize_auth("test-auth").await;

        // Record some results
        collector.record_result("test-auth", true, 100.0, 200).await;
        collector.record_result("test-auth", true, 150.0, 200).await;
        collector
            .record_result("test-auth", false, 200.0, 500)
            .await;

        let metrics = collector.get_metrics("test-auth").await.unwrap();
        assert_eq!(metrics.total_requests, 3);
        assert_eq!(metrics.success_count, 2);
        assert_eq!(metrics.failure_count, 1);
        assert!(metrics.avg_latency_ms > 0.0);
        assert!(metrics.success_rate > 0.0 && metrics.success_rate < 1.0);
    }

    #[tokio::test]
    async fn test_ewma_calculation() {
        let collector = MetricsCollector::new();
        collector.initialize_auth("test-auth").await;

        // Record results with varying latencies
        collector.record_result("test-auth", true, 100.0, 200).await;
        collector.record_result("test-auth", true, 200.0, 200).await;
        collector.record_result("test-auth", true, 300.0, 200).await;

        let metrics = collector.get_metrics("test-auth").await.unwrap();
        // EWMA should smooth the values
        assert!(metrics.avg_latency_ms > 100.0 && metrics.avg_latency_ms < 300.0);
    }

    #[tokio::test]
    async fn test_consecutive_tracking() {
        let collector = MetricsCollector::new();
        collector.initialize_auth("test-auth").await;

        collector.record_result("test-auth", true, 100.0, 200).await;
        collector.record_result("test-auth", true, 100.0, 200).await;
        collector
            .record_result("test-auth", false, 100.0, 500)
            .await;

        let metrics = collector.get_metrics("test-auth").await.unwrap();
        assert_eq!(metrics.consecutive_failures, 1);
        assert_eq!(metrics.consecutive_successes, 0);

        collector.record_result("test-auth", true, 100.0, 200).await;
        let metrics = collector.get_metrics("test-auth").await.unwrap();
        assert_eq!(metrics.consecutive_failures, 0);
        assert_eq!(metrics.consecutive_successes, 1);
    }

    #[tokio::test]
    async fn test_empty_auth_id() {
        let collector = MetricsCollector::new();

        // Initialize with empty auth should be a no-op
        collector.initialize_auth("").await;
        assert!(collector.get_metrics("").await.is_none());

        // Record with empty auth should be a no-op
        collector.record_result("", true, 100.0, 200).await;
        assert!(collector.get_metrics("").await.is_none());

        // Get metrics with empty auth should return None
        assert!(collector.get_metrics("").await.is_none());
    }

    #[tokio::test]
    async fn test_zero_latency() {
        let collector = MetricsCollector::new();
        collector.initialize_auth("test-auth").await;

        // Record result with zero latency
        collector.record_result("test-auth", true, 0.0, 200).await;

        let metrics = collector.get_metrics("test-auth").await.unwrap();
        // Zero latency should not affect min/max/avg
        assert_eq!(metrics.avg_latency_ms, 0.0);
        assert_eq!(metrics.min_latency_ms, f64::MAX);
        assert_eq!(metrics.max_latency_ms, 0.0);
    }

    #[tokio::test]
    async fn test_cleanup_triggered_by_operations() {
        let collector = MetricsCollector::with_limit(5);

        // Add more entries than the limit
        for i in 0..10 {
            collector.initialize_auth(&format!("auth-{}", i)).await;
        }

        // Cleanup should have removed oldest entries
        let all_metrics = collector.get_all_metrics().await;
        assert!(all_metrics.len() <= 5);
    }

    #[tokio::test]
    async fn test_concurrent_recording() {
        let collector = std::sync::Arc::new(MetricsCollector::new());
        let mut handles = vec![];

        // Spawn multiple concurrent recorders
        for i in 0..10 {
            let collector_clone = collector.clone();
            let handle = tokio::spawn(async move {
                for j in 0..10 {
                    collector_clone
                        .record_result(&format!("auth-{}", i), true, 100.0 + j as f64, 200)
                        .await;
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all metrics were recorded
        let all_metrics = collector.get_all_metrics().await;
        assert_eq!(all_metrics.len(), 10);

        for i in 0..10 {
            let metrics = collector.get_metrics(&format!("auth-{}", i)).await.unwrap();
            assert_eq!(metrics.total_requests, 10);
            assert_eq!(metrics.success_count, 10);
        }
    }

    #[tokio::test]
    async fn test_reset_functionality() {
        let collector = MetricsCollector::new();
        collector.initialize_auth("test-auth").await;

        // Record some results
        collector.record_result("test-auth", true, 100.0, 200).await;
        collector.record_result("test-auth", true, 150.0, 200).await;

        // Verify metrics exist
        let metrics = collector.get_metrics("test-auth").await.unwrap();
        assert_eq!(metrics.total_requests, 2);

        // Reset metrics
        collector.reset("test-auth").await;

        // Verify metrics are reset
        let metrics = collector.get_metrics("test-auth").await.unwrap();
        assert_eq!(metrics.total_requests, 0);
        assert_eq!(metrics.success_count, 0);
        assert_eq!(metrics.failure_count, 0);
        assert_eq!(metrics.consecutive_successes, 0);
        assert_eq!(metrics.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn test_reset_all() {
        let collector = MetricsCollector::new();

        // Add multiple auths
        collector.initialize_auth("auth-1").await;
        collector.initialize_auth("auth-2").await;
        collector.initialize_auth("auth-3").await;

        // Verify all exist
        let all_metrics = collector.get_all_metrics().await;
        assert_eq!(all_metrics.len(), 3);

        // Reset all
        collector.reset_all().await;

        // Verify all are cleared
        let all_metrics = collector.get_all_metrics().await;
        assert_eq!(all_metrics.len(), 0);
    }

    #[tokio::test]
    async fn test_min_max_latency_tracking() {
        let collector = MetricsCollector::new();
        collector.initialize_auth("test-auth").await;

        // Record varying latencies
        collector.record_result("test-auth", true, 50.0, 200).await;
        collector.record_result("test-auth", true, 150.0, 200).await;
        collector.record_result("test-auth", true, 100.0, 200).await;

        let metrics = collector.get_metrics("test-auth").await.unwrap();
        assert_eq!(metrics.min_latency_ms, 50.0);
        assert_eq!(metrics.max_latency_ms, 150.0);
    }

    #[tokio::test]
    async fn test_metrics_with_limit_zero() {
        let collector = MetricsCollector::with_limit(0);

        // Should use default limit
        collector.initialize_auth("test-auth").await;
        assert!(collector.get_metrics("test-auth").await.is_some());
    }

    #[tokio::test]
    async fn test_clone_creates_independent_metrics() {
        let collector1 = MetricsCollector::new();
        collector1.initialize_auth("auth-1").await;

        let collector2 = collector1.clone();

        // Clone should have independent metrics storage
        collector2.initialize_auth("auth-2").await;

        // Collector1 should not see auth-2
        assert!(collector1.get_metrics("auth-2").await.is_none());

        // Collector2 should have auth-2
        assert!(collector2.get_metrics("auth-2").await.is_some());
    }

    #[tokio::test]
    async fn test_auto_initialize_on_record() {
        let collector = MetricsCollector::new();

        // Record without explicit initialize
        collector.record_result("auto-auth", true, 100.0, 200).await;

        // Should auto-create metrics
        let metrics = collector.get_metrics("auto-auth").await.unwrap();
        assert_eq!(metrics.total_requests, 1);
        assert_eq!(metrics.success_count, 1);
    }
}
