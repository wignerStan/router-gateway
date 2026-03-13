use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    metrics: Arc<tokio::sync::RwLock<std::collections::HashMap<String, AuthMetrics>>>,
    max_entries: usize,
    cleanup_interval: i64,
    op_count: std::sync::Arc<std::sync::atomic::AtomicI64>,
}

impl Clone for MetricsCollector {
    fn clone(&self) -> Self {
        Self {
            metrics: Arc::clone(&self.metrics),
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
            metrics: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            max_entries: 10_000,
            cleanup_interval: 100,
            op_count: std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0)),
        }
    }

    /// Create a metrics collector with a limit
    pub fn with_limit(max_entries: usize) -> Self {
        Self {
            metrics: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
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
    async fn test_clone_shares_metrics_storage() {
        let collector1 = MetricsCollector::new();
        collector1.initialize_auth("auth-1").await;

        let collector2 = collector1.clone();

        // Clone shares the same metrics storage via Arc
        collector2.initialize_auth("auth-2").await;

        // Collector1 should see auth-2 (shared state)
        assert!(collector1.get_metrics("auth-2").await.is_some());

        // Collector2 should see auth-1 (shared state)
        assert!(collector2.get_metrics("auth-1").await.is_some());
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

    // ============================================================
    // Edge Case Tests for Metrics Collector
    // ============================================================

    #[tokio::test]
    async fn test_metrics_with_zero_requests() {
        let collector = MetricsCollector::new();
        collector.initialize_auth("test-auth").await;

        let metrics = collector.get_metrics("test-auth").await.unwrap();

        assert_eq!(metrics.total_requests, 0);
        assert_eq!(metrics.success_count, 0);
        assert_eq!(metrics.failure_count, 0);
        assert_eq!(metrics.consecutive_successes, 0);
        assert_eq!(metrics.consecutive_failures, 0);
        // Initial success rate is 1.0 (optimistic default)
        assert_eq!(metrics.success_rate, 1.0);
        assert_eq!(metrics.error_rate, 0.0);
    }

    #[tokio::test]
    async fn test_latency_rolling_average_with_single_request() {
        let collector = MetricsCollector::new();
        collector.initialize_auth("test-auth").await;

        // Single latency record
        collector.record_result("test-auth", true, 150.0, 200).await;

        let metrics = collector.get_metrics("test-auth").await.unwrap();

        // With single request, avg should equal that request's latency
        assert_eq!(metrics.avg_latency_ms, 150.0);
        assert_eq!(metrics.min_latency_ms, 150.0);
        assert_eq!(metrics.max_latency_ms, 150.0);
    }

    #[tokio::test]
    async fn test_success_rate_calculation_all_failures() {
        let collector = MetricsCollector::new();
        collector.initialize_auth("test-auth").await;

        // Record many failures
        for _ in 0..50 {
            collector
                .record_result("test-auth", false, 100.0, 500)
                .await;
        }

        let metrics = collector.get_metrics("test-auth").await.unwrap();

        assert_eq!(metrics.total_requests, 50);
        assert_eq!(metrics.failure_count, 50);
        assert_eq!(metrics.success_count, 0);
        assert_eq!(metrics.consecutive_failures, 50);
        assert_eq!(metrics.consecutive_successes, 0);

        // Success rate should be low (EWMA weighted, not 0.0)
        // With EWMA_ALPHA = 0.3, after many failures it trends toward 0
        assert!(
            metrics.success_rate < 0.1,
            "Success rate should be very low after all failures: {}",
            metrics.success_rate
        );
        assert!(
            metrics.error_rate > 0.9,
            "Error rate should be very high: {}",
            metrics.error_rate
        );
    }

    #[tokio::test]
    async fn test_concurrent_metric_recording_no_data_loss() {
        let collector = std::sync::Arc::new(MetricsCollector::new());
        let mut handles = vec![];

        // Spawn many concurrent recorders for the SAME auth (tests locking)
        for _ in 0..10 {
            let collector_clone = collector.clone();
            let handle = tokio::spawn(async move {
                for _ in 0..100 {
                    collector_clone
                        .record_result("shared-auth", true, 50.0, 200)
                        .await;
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all metrics were recorded
        let metrics = collector.get_metrics("shared-auth").await.unwrap();
        assert_eq!(
            metrics.total_requests, 1000,
            "All 1000 requests should be recorded"
        );
        assert_eq!(metrics.success_count, 1000);
        assert_eq!(metrics.failure_count, 0);
    }

    #[tokio::test]
    async fn test_metrics_reset_clears_all_data() {
        let collector = MetricsCollector::new();
        collector.initialize_auth("test-auth").await;

        // Record various metrics
        for _ in 0..50 {
            collector.record_result("test-auth", true, 100.0, 200).await;
        }
        for _ in 0..10 {
            collector
                .record_result("test-auth", false, 200.0, 500)
                .await;
        }

        // Verify data exists
        let before = collector.get_metrics("test-auth").await.unwrap();
        assert_eq!(before.total_requests, 60);

        // Reset
        collector.reset("test-auth").await;

        // Verify all reset
        let after = collector.get_metrics("test-auth").await.unwrap();
        assert_eq!(after.total_requests, 0);
        assert_eq!(after.success_count, 0);
        assert_eq!(after.failure_count, 0);
        assert_eq!(after.consecutive_successes, 0);
        assert_eq!(after.consecutive_failures, 0);
        assert_eq!(after.avg_latency_ms, 0.0);
        assert_eq!(after.min_latency_ms, f64::MAX);
        assert_eq!(after.max_latency_ms, 0.0);
    }

    #[tokio::test]
    async fn test_ewma_latency_smoothing() {
        let collector = MetricsCollector::new();
        collector.initialize_auth("test-auth").await;

        // Record latencies with large jump
        collector.record_result("test-auth", true, 100.0, 200).await;

        // Now record a much higher latency
        collector
            .record_result("test-auth", true, 1000.0, 200)
            .await;
        let m2 = collector.get_metrics("test-auth").await.unwrap();

        // EWMA should smooth the jump (not immediately jump to 1000)
        // EWMA = 0.3 * new + 0.7 * old
        let expected = 0.3 * 1000.0 + 0.7 * 100.0;
        assert!(
            (m2.avg_latency_ms - expected).abs() < 0.1,
            "EWMA should smooth latency: expected {}, got {}",
            expected,
            m2.avg_latency_ms
        );
        assert!(
            m2.avg_latency_ms < 1000.0,
            "EWMA should not jump immediately to new value"
        );
    }

    #[tokio::test]
    async fn test_ewma_success_rate_smoothing() {
        let collector = MetricsCollector::new();
        collector.initialize_auth("test-auth").await;

        // Start with success (initial rate is 1.0)
        collector.record_result("test-auth", true, 100.0, 200).await;
        let m1 = collector.get_metrics("test-auth").await.unwrap();

        // Now record failure
        collector
            .record_result("test-auth", false, 100.0, 500)
            .await;
        let m2 = collector.get_metrics("test-auth").await.unwrap();

        // Success rate should decrease smoothly
        // EWMA = 0.3 * 0 + 0.7 * previous
        let expected = 0.3 * 0.0 + 0.7 * m1.success_rate;
        assert!(
            (m2.success_rate - expected).abs() < 0.1,
            "EWMA should smooth success rate"
        );
    }

    #[tokio::test]
    async fn test_latency_extreme_values() {
        let collector = MetricsCollector::new();
        collector.initialize_auth("test-auth").await;

        // Very small latency
        collector
            .record_result("test-auth", true, f64::MIN_POSITIVE, 200)
            .await;
        let m1 = collector.get_metrics("test-auth").await.unwrap();
        assert!(m1.avg_latency_ms > 0.0);

        // Very large latency
        collector
            .record_result("test-auth", true, 1_000_000.0, 200)
            .await;
        let m2 = collector.get_metrics("test-auth").await.unwrap();
        assert!(m2.max_latency_ms == 1_000_000.0);
        assert!(m2.min_latency_ms == f64::MIN_POSITIVE);
    }

    #[tokio::test]
    async fn test_negative_latency_ignored() {
        let collector = MetricsCollector::new();
        collector.initialize_auth("test-auth").await;

        // Record valid latency first
        collector.record_result("test-auth", true, 100.0, 200).await;

        // Try to record negative latency (should be ignored per > 0.0 check)
        collector.record_result("test-auth", true, -50.0, 200).await;

        let metrics = collector.get_metrics("test-auth").await.unwrap();

        // Total requests still increments
        assert_eq!(metrics.total_requests, 2);
        // But negative latency should not affect min/max/avg
        assert_eq!(metrics.min_latency_ms, 100.0);
        assert_eq!(metrics.max_latency_ms, 100.0);
    }
}
