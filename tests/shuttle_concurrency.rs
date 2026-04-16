//! Shuttle-based randomized concurrency tests for async gateway patterns.
//!
//! Models `MetricsCollector` and `HealthManager` concurrent access patterns
//! using shuttle's sync primitives. Tests for deadlocks, livelocks, and
//! assertion failures under randomized async scheduling.
//!
//! Run with: `cargo test --test shuttle_concurrency`

use std::collections::HashMap;
use std::sync::Arc;

use shuttle::sync::RwLock;

// ============================================================
// Models
// ============================================================

/// Model of `MetricsCollector` using shuttle's `RwLock`.
mod metrics_model {
    use super::*;

    #[derive(Clone, Debug)]
    pub struct Metrics {
        pub total_requests: i64,
        pub success_count: i64,
        pub failure_count: i64,
    }

    pub struct ShuttleMetricsCollector {
        metrics: Arc<RwLock<HashMap<String, Metrics>>>,
    }

    impl ShuttleMetricsCollector {
        pub fn new() -> Self {
            Self {
                metrics: Arc::new(RwLock::new(HashMap::new())),
            }
        }

        // Guard scope mirrors production MetricsCollector pattern.
        #[allow(clippy::significant_drop_tightening)]
        pub fn record_result(&self, auth_id: &str, success: bool) {
            let mut metrics = self.metrics.write().unwrap();
            let entry = metrics.entry(auth_id.to_string()).or_insert(Metrics {
                total_requests: 0,
                success_count: 0,
                failure_count: 0,
            });
            entry.total_requests += 1;
            if success {
                entry.success_count += 1;
            } else {
                entry.failure_count += 1;
            }
        }

        pub fn get_metrics(&self, auth_id: &str) -> Option<Metrics> {
            let metrics = self.metrics.read().unwrap();
            metrics.get(auth_id).cloned()
        }

        pub fn get_all_count(&self) -> usize {
            let metrics = self.metrics.read().unwrap();
            metrics.len()
        }
    }
}

/// Model of `HealthManager` using shuttle's `RwLock`.
mod health_model {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum HealthStatus {
        Healthy,
        Degraded,
        Unhealthy,
    }

    pub struct AuthHealth {
        pub status: HealthStatus,
        pub consecutive_failures: i32,
    }

    pub struct ShuttleHealthManager {
        health: Arc<RwLock<HashMap<String, AuthHealth>>>,
        unhealthy_threshold: i32,
    }

    impl ShuttleHealthManager {
        pub fn new(unhealthy_threshold: i32) -> Self {
            Self {
                health: Arc::new(RwLock::new(HashMap::new())),
                unhealthy_threshold,
            }
        }

        // Guard scope mirrors production HealthManager pattern.
        #[allow(clippy::significant_drop_tightening)]
        pub fn update_from_result(&self, auth_id: &str, success: bool) {
            let mut health = self.health.write().unwrap();
            let entry = health.entry(auth_id.to_string()).or_insert(AuthHealth {
                status: HealthStatus::Healthy,
                consecutive_failures: 0,
            });

            if success {
                entry.consecutive_failures = 0;
                entry.status = HealthStatus::Healthy;
            } else {
                entry.consecutive_failures += 1;
                if entry.consecutive_failures >= self.unhealthy_threshold {
                    entry.status = HealthStatus::Unhealthy;
                } else {
                    entry.status = HealthStatus::Degraded;
                }
            }
        }

        pub fn get_status(&self, auth_id: &str) -> HealthStatus {
            let health = self.health.read().unwrap();
            health
                .get(auth_id)
                .map_or(HealthStatus::Healthy, |h| h.status)
        }
    }
}

// ============================================================
// Metrics Collector Tests
// ============================================================

#[test]
fn metrics_concurrent_record_no_loss() {
    shuttle::check_random(
        || {
            shuttle::future::block_on(async {
                let collector = Arc::new(metrics_model::ShuttleMetricsCollector::new());

                let mut tasks = Vec::new();
                for i in 0..5 {
                    let c = Arc::clone(&collector);
                    tasks.push(shuttle::future::spawn(async move {
                        for j in 0..10 {
                            c.record_result(&format!("auth-{i}"), j % 2 == 0);
                        }
                    }));
                }

                for task in tasks {
                    let _ = task.await;
                }

                assert_eq!(collector.get_all_count(), 5);
                for i in 0..5 {
                    let m = collector.get_metrics(&format!("auth-{i}")).unwrap();
                    assert_eq!(m.total_requests, 10);
                    assert_eq!(m.success_count + m.failure_count, 10);
                }
            });
        },
        1000,
    );
}

#[test]
fn metrics_concurrent_same_auth_contention() {
    shuttle::check_random(
        || {
            shuttle::future::block_on(async {
                let collector = Arc::new(metrics_model::ShuttleMetricsCollector::new());

                let mut tasks = Vec::new();
                for _ in 0..10 {
                    let c = Arc::clone(&collector);
                    tasks.push(shuttle::future::spawn(async move {
                        for _ in 0..100 {
                            c.record_result("shared-auth", true);
                        }
                    }));
                }

                for task in tasks {
                    let _ = task.await;
                }

                let m = collector.get_metrics("shared-auth").unwrap();
                assert_eq!(m.total_requests, 1000);
                assert_eq!(m.success_count, 1000);
                assert_eq!(m.failure_count, 0);
            });
        },
        500,
    );
}

// ============================================================
// Health Manager Tests
// ============================================================

#[test]
fn health_concurrent_transitions_no_deadlock() {
    shuttle::check_random(
        || {
            shuttle::future::block_on(async {
                let manager = Arc::new(health_model::ShuttleHealthManager::new(3));

                let mut tasks = Vec::new();
                for i in 0..8 {
                    let m = Arc::clone(&manager);
                    tasks.push(shuttle::future::spawn(async move {
                        for j in 0..20 {
                            m.update_from_result("auth-1", j % (i + 1) == 0);
                        }
                    }));
                }

                for _ in 0..4 {
                    let m = Arc::clone(&manager);
                    tasks.push(shuttle::future::spawn(async move {
                        for _ in 0..20 {
                            let status = m.get_status("auth-1");
                            assert!(
                                status == health_model::HealthStatus::Healthy
                                    || status == health_model::HealthStatus::Degraded
                                    || status == health_model::HealthStatus::Unhealthy
                            );
                        }
                    }));
                }

                for task in tasks {
                    let _ = task.await;
                }
            });
        },
        1000,
    );
}

#[test]
fn health_recovery_under_contention() {
    shuttle::check_random(
        || {
            shuttle::future::block_on(async {
                let manager = Arc::new(health_model::ShuttleHealthManager::new(3));

                // Drive unhealthy
                for _ in 0..5 {
                    manager.update_from_result("auth-x", false);
                }
                assert_eq!(
                    manager.get_status("auth-x"),
                    health_model::HealthStatus::Unhealthy
                );

                // Concurrent recovery
                let mut tasks = Vec::new();
                for _ in 0..5 {
                    let m = Arc::clone(&manager);
                    tasks.push(shuttle::future::spawn(async move {
                        for _ in 0..5 {
                            m.update_from_result("auth-x", true);
                        }
                    }));
                }
                for task in tasks {
                    let _ = task.await;
                }

                assert_eq!(
                    manager.get_status("auth-x"),
                    health_model::HealthStatus::Healthy
                );
            });
        },
        500,
    );
}

#[test]
fn no_deadlock_under_read_write_contention() {
    shuttle::check_random(
        || {
            shuttle::future::block_on(async {
                let collector = Arc::new(metrics_model::ShuttleMetricsCollector::new());
                let manager = Arc::new(health_model::ShuttleHealthManager::new(3));

                let mut tasks = Vec::new();
                for i in 0..10 {
                    let c = Arc::clone(&collector);
                    let m = Arc::clone(&manager);
                    tasks.push(shuttle::future::spawn(async move {
                        let auth_id = format!("auth-{i}");
                        for j in 0..20 {
                            let success = j % 3 != 0;
                            c.record_result(&auth_id, success);
                            m.update_from_result(&auth_id, success);
                            let _ = c.get_metrics(&auth_id);
                            let _ = m.get_status(&auth_id);
                        }
                    }));
                }

                for task in tasks {
                    let _ = task.await;
                }
            });
        },
        1000,
    );
}
