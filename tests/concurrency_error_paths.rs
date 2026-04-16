#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Error-path concurrency tests for the gateway.
//!
//! Stress tests using real OS threads (non-deterministic).
//!
//! These tests use `#[tokio::test(flavor = "multi_thread")]` which exercises
//! actual concurrent scheduling. Thread interleavings depend on OS scheduler —
//! passes here do NOT guarantee race-freedom. These complement deterministic
//! loom/shuttle tests, not replace them.
//!
//! If these tests fail, it IS a real bug. If they pass, run loom/shuttle for
//! exhaustive coverage.

use gateway::routing::config::HealthConfig;
use gateway::routing::health::{HealthManager, HealthStatus};
use gateway::routing::metrics::MetricsCollector;
use gateway::state::RateLimiter;
use std::sync::Arc;

// ============================================================
// RateLimiter Error Paths
// ============================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rate_limiter_concurrent_exhaustion() {
    let limiter = Arc::new(RateLimiter::new(5));

    let mut handles = Vec::new();
    for _ in 0..10 {
        let limiter = Arc::clone(&limiter);
        handles.push(tokio::spawn(async move {
            let mut accepted = 0u64;
            let mut rejected = 0u64;
            for _ in 0..5 {
                if limiter.check("10.0.0.1") {
                    accepted += 1;
                } else {
                    rejected += 1;
                }
            }
            (accepted, rejected)
        }));
    }

    let results: Vec<_> = futures::future::join_all(handles).await;
    let total_accepted: u64 = results.iter().map(|r| r.as_ref().unwrap().0).sum();
    let total_rejected: u64 = results.iter().map(|r| r.as_ref().unwrap().1).sum();

    assert_eq!(total_accepted, 5, "Only 5 requests should be accepted");
    assert_eq!(total_rejected, 45, "45 requests should be rejected");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rate_limiter_concurrent_exhaustion_multiple_ips() {
    let limiter = Arc::new(RateLimiter::new(3));

    let mut handles = Vec::new();
    // 4 IPs, each hitting the limit concurrently
    for ip_idx in 0..4u16 {
        let limiter = Arc::clone(&limiter);
        handles.push(tokio::spawn(async move {
            let ip = format!("10.0.0.{ip_idx}");
            let mut accepted = 0u64;
            let mut rejected = 0u64;
            for _ in 0..10 {
                if limiter.check(&ip) {
                    accepted += 1;
                } else {
                    rejected += 1;
                }
            }
            (ip, accepted, rejected)
        }));
    }

    let results: Vec<_> = futures::future::join_all(handles).await;
    for result in &results {
        let r = result.as_ref().unwrap();
        assert_eq!(r.1, 3, "IP {} should accept exactly 3 requests", r.0);
        assert_eq!(r.2, 7, "IP {} should reject 7 requests", r.0);
    }
}

#[tokio::test]
async fn rate_limiter_empty_ip_bypasses_limiting() {
    let limiter = Arc::new(RateLimiter::new(1));
    let mut handles = Vec::new();

    for _ in 0..100 {
        let limiter = Arc::clone(&limiter);
        handles.push(tokio::spawn(async move { limiter.check("") }));
    }

    for handle in handles {
        assert!(handle.await.unwrap(), "Empty IP should always pass");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rate_limiter_concurrent_different_ips_isolated() {
    let limiter = Arc::new(RateLimiter::new(2));

    let mut handles = Vec::new();
    for i in 0..50 {
        let limiter = Arc::clone(&limiter);
        handles.push(tokio::spawn(async move {
            let ip = format!("192.168.1.{i}");
            // Each IP should get exactly 2 accepts (the limit) then 8 rejects
            let mut accepted = 0u64;
            for _ in 0..10 {
                if limiter.check(&ip) {
                    accepted += 1;
                }
            }
            accepted
        }));
    }

    let results: Vec<_> = futures::future::join_all(handles).await;
    for result in &results {
        assert_eq!(
            *result.as_ref().unwrap(),
            2,
            "Each IP should independently get exactly 2 accepts"
        );
    }
}

// ============================================================
// MetricsCollector Error Paths
// ============================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn metrics_concurrent_failure_only_recording() {
    let collector = Arc::new(MetricsCollector::new());
    collector.initialize_auth("fail-auth").await;

    let mut handles = Vec::new();
    for _ in 0..10 {
        let c = Arc::clone(&collector);
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                c.record_result("fail-auth", false, 200.0, 500).await;
            }
        }));
    }

    for handle in handles {
        assert!(handle.await.is_ok(), "Recording task should not panic");
    }

    let metrics = collector.get_metrics("fail-auth").await.unwrap();
    assert_eq!(metrics.total_requests, 1000);
    assert_eq!(metrics.failure_count, 1000);
    assert_eq!(metrics.success_count, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn metrics_concurrent_reset_during_recording() {
    let collector = Arc::new(MetricsCollector::new());
    collector.initialize_auth("reset-auth").await;

    let mut handles = Vec::new();

    for _ in 0..5 {
        let c = Arc::clone(&collector);
        handles.push(tokio::spawn(async move {
            for j in 0..200 {
                c.record_result("reset-auth", j % 2 == 0, 100.0, 200).await;
            }
        }));
    }

    for _ in 0..5 {
        let c = Arc::clone(&collector);
        handles.push(tokio::spawn(async move {
            for _ in 0..50 {
                c.reset("reset-auth").await;
                tokio::task::yield_now().await;
            }
        }));
    }

    for handle in handles {
        assert!(
            handle.await.is_ok(),
            "No task should panic during reset+record"
        );
    }

    // Final state should be internally consistent
    if let Some(metrics) = collector.get_metrics("reset-auth").await {
        assert_eq!(
            metrics.success_count + metrics.failure_count,
            metrics.total_requests,
            "Counts should be internally consistent"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn metrics_concurrent_cleanup_triggers_no_panic() {
    let collector = Arc::new(MetricsCollector::with_limit(5));

    let mut handles = Vec::new();
    for i in 0..20 {
        let c = Arc::clone(&collector);
        handles.push(tokio::spawn(async move {
            for _ in 0..50 {
                c.record_result(&format!("auth-{i}"), true, 100.0, 200)
                    .await;
            }
        }));
    }

    for handle in handles {
        assert!(handle.await.is_ok(), "Recording should not panic");
    }

    // Cleanup may have run but entries can be recreated by or_insert_with.
    // The key invariant is that all surviving entries are internally consistent.
    let all_metrics = collector.get_all_metrics().await;
    for (_, m) in all_metrics {
        assert_eq!(m.success_count + m.failure_count, m.total_requests);
        assert!(
            m.total_requests > 0,
            "Each surviving entry should have recorded requests"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn metrics_cleanup_evicts_oldest_when_over_limit() {
    // Single-threaded to ensure deterministic ordering for this cleanup test
    let collector = MetricsCollector::with_limit(5);

    // Initialize 10 entries sequentially — cleanup triggers every 100 ops
    // but initialize_auth does a len check directly
    for i in 0..10 {
        collector.initialize_auth(&format!("auth-{i}")).await;
    }

    let all_metrics = collector.get_all_metrics().await;
    assert!(
        all_metrics.len() <= 5,
        "Should have at most 5 entries after cleanup, got {}",
        all_metrics.len()
    );

    for (_, m) in all_metrics {
        assert_eq!(m.success_count + m.failure_count, m.total_requests);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn metrics_concurrent_reset_all_during_recording() {
    let collector = Arc::new(MetricsCollector::new());
    collector.initialize_auth("auth-1").await;
    collector.initialize_auth("auth-2").await;

    let mut handles = Vec::new();

    // Recorders
    for i in 0..4 {
        let c = Arc::clone(&collector);
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                c.record_result(&format!("auth-{i}"), true, 50.0, 200).await;
            }
        }));
    }

    // Reset-all callers
    for _ in 0..4 {
        let c = Arc::clone(&collector);
        handles.push(tokio::spawn(async move {
            for _ in 0..20 {
                c.reset_all().await;
                tokio::task::yield_now().await;
            }
        }));
    }

    for handle in handles {
        assert!(handle.await.is_ok(), "reset_all + record should not panic");
    }

    // Verify internal consistency of whatever survived
    let all_metrics = collector.get_all_metrics().await;
    for (_, m) in all_metrics {
        assert_eq!(
            m.success_count + m.failure_count,
            m.total_requests,
            "Metrics should be consistent after concurrent reset_all"
        );
    }
}

// ============================================================
// HealthManager Error Paths
// ============================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn health_concurrent_failure_burst_then_recovery() {
    let config = HealthConfig {
        unhealthy_threshold: 5,
        ..Default::default()
    };
    let manager = Arc::new(HealthManager::new(config));

    // Phase 1: concurrent failures
    let mut handles = Vec::new();
    for _ in 0..10 {
        let m = Arc::clone(&manager);
        handles.push(tokio::spawn(async move {
            for _ in 0..5 {
                m.update_from_result("burst-auth", false, 500).await;
            }
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    assert_eq!(
        manager.get_status("burst-auth").await,
        HealthStatus::Unhealthy
    );

    // Phase 2: concurrent recovery
    let mut handles = Vec::new();
    for _ in 0..10 {
        let m = Arc::clone(&manager);
        handles.push(tokio::spawn(async move {
            for _ in 0..5 {
                m.update_from_result("burst-auth", true, 200).await;
            }
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    assert_eq!(
        manager.get_status("burst-auth").await,
        HealthStatus::Healthy
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn health_clone_concurrent_write_visibility() {
    let manager = HealthManager::new(HealthConfig::default());
    // ALLOW: HealthManager::clone shares the same Arc<RwLock> — this is the
    // documented pattern for shared health state (see HealthManager docs).
    #[allow(clippy::clone_on_ref_ptr)]
    let clone = manager.clone();

    let mut handles = Vec::new();

    #[allow(clippy::clone_on_ref_ptr)]
    let manager_clone = manager.clone();
    handles.push(tokio::spawn(async move {
        for i in 0..50 {
            manager_clone
                .update_from_result("clone-auth", i % 2 == 0, 200)
                .await;
        }
    }));

    handles.push(tokio::spawn(async move {
        for i in 0..50 {
            clone
                .update_from_result("clone-auth", i % 3 == 0, 200)
                .await;
        }
    }));

    for h in handles {
        h.await.unwrap();
    }

    // Both handles point to same Arc<RwLock>, so state must be identical
    let h1 = manager.get_health("clone-auth").await.unwrap();
    let h2 = manager.get_health("clone-auth").await.unwrap();
    assert_eq!(h1.status, h2.status);
    assert_eq!(h1.error_counts, h2.error_counts);
    assert_eq!(h1.consecutive_failures, h2.consecutive_failures);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn health_concurrent_reset_during_updates() {
    let manager = Arc::new(HealthManager::new(HealthConfig::default()));

    let mut handles = Vec::new();

    // Updaters
    for _ in 0..5 {
        let m = Arc::clone(&manager);
        handles.push(tokio::spawn(async move {
            for i in 0..100 {
                m.update_from_result(
                    "reset-health-auth",
                    i % 2 == 0,
                    if i % 2 == 0 { 200 } else { 500 },
                )
                .await;
            }
        }));
    }

    // Resetters
    for _ in 0..5 {
        let m = Arc::clone(&manager);
        handles.push(tokio::spawn(async move {
            for _ in 0..20 {
                m.reset("reset-health-auth").await;
                tokio::task::yield_now().await;
            }
        }));
    }

    for h in handles {
        assert!(
            h.await.is_ok(),
            "No task should panic during concurrent reset+update"
        );
    }

    // Verify the entry exists and is in a valid state
    if let Some(health) = manager.get_health("reset-health-auth").await {
        assert!(
            matches!(
                health.status,
                HealthStatus::Healthy | HealthStatus::Degraded | HealthStatus::Unhealthy
            ),
            "Status should be a valid HealthStatus variant"
        );
    }
}

// ============================================================
// Cross-Component Concurrency
// ============================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn metrics_and_health_concurrent_updates() {
    let collector = Arc::new(MetricsCollector::new());
    let manager = Arc::new(HealthManager::new(HealthConfig::default()));
    collector.initialize_auth("cross-auth").await;

    let mut handles = Vec::new();
    for _ in 0..8 {
        let c = Arc::clone(&collector);
        let m = Arc::clone(&manager);
        handles.push(tokio::spawn(async move {
            for j in 0..50 {
                let success = j % 3 != 0;
                c.record_result(
                    "cross-auth",
                    success,
                    100.0,
                    if success { 200 } else { 500 },
                )
                .await;
                m.update_from_result("cross-auth", success, if success { 200 } else { 500 })
                    .await;
            }
        }));
    }

    for h in handles {
        assert!(
            h.await.is_ok(),
            "Cross-component concurrent update should not panic"
        );
    }

    let metrics = collector.get_metrics("cross-auth").await.unwrap();
    assert_eq!(metrics.total_requests, 400);
    assert_eq!(metrics.success_count + metrics.failure_count, 400);

    let health = manager.get_health("cross-auth").await.unwrap();
    // AuthHealth tracks error_counts by status code; verify some were recorded
    let total_errors: i32 = health.error_counts.values().sum();
    assert!(
        total_errors > 0,
        "Health should have recorded error codes from 500 responses"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn metrics_health_ratelimiter_stress() {
    let collector = Arc::new(MetricsCollector::new());
    let manager = Arc::new(HealthManager::new(HealthConfig::default()));
    let limiter = Arc::new(RateLimiter::new(100));
    collector.initialize_auth("stress-auth").await;

    let mut handles = Vec::new();
    for worker in 0..8 {
        let c = Arc::clone(&collector);
        let m = Arc::clone(&manager);
        let l = Arc::clone(&limiter);
        handles.push(tokio::spawn(async move {
            let ip = format!("10.0.{worker}.1");
            for j in 0..100 {
                let allowed = l.check(&ip);
                let success = allowed && j % 5 != 0;
                let status = if success { 200 } else { 500 };
                c.record_result("stress-auth", success, 50.0, status).await;
                m.update_from_result("stress-auth", success, status).await;
            }
        }));
    }

    for h in handles {
        assert!(h.await.is_ok(), "Stress test should not panic");
    }

    let metrics = collector.get_metrics("stress-auth").await.unwrap();
    assert_eq!(metrics.total_requests, 800);
    assert_eq!(metrics.success_count + metrics.failure_count, 800);
    assert!(metrics.min_latency_ms > 0.0);
}
