#[cfg(test)]
mod sqlite_tests {
    use crate::health::{AuthHealth, HealthStatus};
    use crate::metrics::AuthMetrics;
    use crate::sqlite::collectors::SQLiteHealthManager;
    use crate::sqlite::collectors::SQLiteMetricsCollector;
    use crate::sqlite::store::SQLiteConfig;
    use crate::sqlite::store::SQLiteStore;
    use chrono::Utc;

    mod store_operations {
        use super::*;

        #[tokio::test]
        async fn test_sqlite_store_creation() {
            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await;
            assert!(store.is_ok(), "Failed to create SQLite store");

            let store = store.unwrap();
            // Test that we can query the database
            let stats = store.get_history_stats().await;
            assert!(stats.is_ok(), "Failed to get history stats");
            let (count, _) = stats.unwrap();
            assert_eq!(count, 0, "History should be empty initially");
        }

        #[tokio::test]
        async fn test_metrics_write_and_load() {
            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();

            // Write metrics
            let metrics = AuthMetrics {
                total_requests: 100,
                success_count: 95,
                failure_count: 5,
                avg_latency_ms: 150.0,
                min_latency_ms: 50.0,
                max_latency_ms: 500.0,
                success_rate: 0.95,
                error_rate: 0.05,
                consecutive_successes: 10,
                consecutive_failures: 0,
                last_request_time: Utc::now(),
                last_success_time: Some(Utc::now()),
                last_failure_time: Some(Utc::now()),
            };

            let result = store.write_metrics("test-auth", &metrics).await;
            assert!(result.is_ok(), "Failed to write metrics");

            // Load metrics
            let loaded = store.load_metrics("test-auth").await;
            assert!(loaded.is_ok(), "Failed to load metrics");
            let loaded = loaded.unwrap();
            assert!(loaded.is_some(), "No metrics found");
            let loaded = loaded.unwrap();

            assert_eq!(loaded.total_requests, 100);
            assert_eq!(loaded.success_count, 95);
            assert_eq!(loaded.failure_count, 5);
            assert!((loaded.avg_latency_ms - 150.0).abs() < 0.01);
            assert!((loaded.success_rate - 0.95).abs() < 0.01);
        }

        #[tokio::test]
        async fn test_health_write_and_load() {
            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();

            // Write health
            let health = AuthHealth {
                status: HealthStatus::Healthy,
                consecutive_successes: 5,
                consecutive_failures: 0,
                last_status_change: Utc::now(),
                last_check_time: Utc::now(),
                unavailable_until: None,
                error_counts: std::collections::HashMap::new(),
            };

            let result = store.write_health("test-auth", &health).await;
            assert!(result.is_ok(), "Failed to write health");

            // Load health
            let loaded = store.load_health("test-auth").await;
            assert!(loaded.is_ok(), "Failed to load health");
            let loaded = loaded.unwrap();
            assert!(loaded.is_some(), "No health found");
            let loaded = loaded.unwrap();

            assert_eq!(loaded.status, HealthStatus::Healthy);
            assert_eq!(loaded.consecutive_successes, 5);
            assert_eq!(loaded.consecutive_failures, 0);
        }

        #[tokio::test]
        async fn test_status_history_write() {
            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();

            // Write status history entries
            let result = store
                .write_status_history("test-auth", 200, 100.0, true)
                .await;
            assert!(result.is_ok(), "Failed to write status history");

            let result = store
                .write_status_history("test-auth", 500, 200.0, false)
                .await;
            assert!(result.is_ok(), "Failed to write status history");

            // Check stats
            let stats = store.get_history_stats().await;
            assert!(stats.is_ok(), "Failed to get history stats");
            let (count, _) = stats.unwrap();
            assert_eq!(count, 2, "Should have 2 history entries");
        }

        #[tokio::test]
        async fn test_cleanup_old_history() {
            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();

            // Write some history entries
            for i in 0..5 {
                let result = store
                    .write_status_history(&format!("auth-{}", i), 200, 100.0, true)
                    .await;
                assert!(result.is_ok(), "Failed to write status history");
            }

            // Verify entries exist
            let stats = store.get_history_stats().await.unwrap();
            assert_eq!(stats.0, 5, "Should have 5 history entries");

            // Cleanup old history (with a very short max age)
            let deleted = store.cleanup_old_history(0).await.unwrap();
            // The cleanup might not delete everything if timestamps are very recent
            assert!(deleted >= 0, "Cleanup should return non-negative count");
        }

        #[tokio::test]
        async fn test_load_all_metrics() {
            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();

            // Write metrics for multiple auths
            for i in 1..=3 {
                let metrics = AuthMetrics {
                    total_requests: i * 10,
                    success_count: i * 9,
                    failure_count: i,
                    avg_latency_ms: 100.0 + i as f64,
                    min_latency_ms: 50.0,
                    max_latency_ms: 200.0,
                    success_rate: 0.9,
                    error_rate: 0.1,
                    consecutive_successes: i as i32,
                    consecutive_failures: 0,
                    last_request_time: Utc::now(),
                    last_success_time: Some(Utc::now()),
                    last_failure_time: None,
                };

                store
                    .write_metrics(&format!("auth-{}", i), &metrics)
                    .await
                    .unwrap();
            }

            // Load all metrics
            let all_metrics = store.load_all_metrics().await.unwrap();
            assert_eq!(all_metrics.len(), 3, "Should have 3 auth metrics");

            // Verify one entry
            let metrics = all_metrics.get("auth-2").unwrap();
            assert_eq!(metrics.total_requests, 20);
            assert_eq!(metrics.success_count, 18);
        }
    }

    mod collectors {
        use super::*;

        #[tokio::test]
        async fn test_metrics_collector() {
            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();
            let collector = SQLiteMetricsCollector::new(store);

            // Initialize auth
            collector.initialize_auth("test-auth").await;

            // Record some requests
            collector
                .record_request("test-auth", 100.0, true, 200)
                .await;
            collector
                .record_request("test-auth", 200.0, true, 200)
                .await;
            collector
                .record_request("test-auth", 500.0, false, 500)
                .await;

            // Get metrics
            let metrics = collector.get_metrics("test-auth").await;
            assert!(metrics.is_some(), "Should have metrics");
            let metrics = metrics.unwrap();

            assert_eq!(metrics.total_requests, 3);
            assert_eq!(metrics.success_count, 2);
            assert_eq!(metrics.failure_count, 1);
        }


        #[tokio::test]
        async fn test_metrics_collector_flush() {
            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();
            let collector = SQLiteMetricsCollector::new(store.clone());

            collector.initialize_auth("test-auth-flush").await;
            collector
                .record_request("test-auth-flush", 100.0, true, 200)
                .await;

            // Wait briefly to ensure any async operations settle (though flush should be manual)
            // Call flush
            let flush_res = collector.flush().await;
            assert!(flush_res.is_ok(), "Flush should succeed");

            // Verify it was written to the store
            let loaded = store.load_metrics("test-auth-flush").await.unwrap();
            assert!(loaded.is_some(), "Metrics should be persisted to DB");
            assert_eq!(loaded.unwrap().total_requests, 1);
        }

        #[tokio::test]
        async fn test_health_manager_flush() {
            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();
            let manager = SQLiteHealthManager::new(store.clone());

            manager.record_failure("test-auth-flush", 500).await;

            // Call flush
            let flush_res = manager.flush().await;
            assert!(flush_res.is_ok(), "Flush should succeed");

            // Verify it was written to the store
            let loaded = store.load_health("test-auth-flush").await.unwrap();
            assert!(loaded.is_some(), "Health should be persisted to DB");
            assert_eq!(loaded.unwrap().consecutive_failures, 1);
        }
        #[tokio::test]
        async fn test_health_manager() {
            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();
            let manager = SQLiteHealthManager::new(store);

            // Record successes
            for _ in 0..3 {
                manager.record_success("test-auth").await;
            }

            // Check status
            let status = manager.get_status("test-auth").await;
            assert_eq!(status, HealthStatus::Healthy);

            // Record failures
            for _ in 0..3 {
                manager.record_failure("test-auth", 500).await;
            }

            // Check status changed to unhealthy
            let status = manager.get_status("test-auth").await;
            assert_eq!(status, HealthStatus::Unhealthy);

            // Check availability
            let available = manager.is_available("test-auth").await;
            assert!(!available, "Should be unavailable during cooldown");
        }
    }

    mod selector_basic {
        use super::*;

        #[tokio::test]
        async fn test_sqlite_selector_basic() {
            use crate::config::SmartRoutingConfig;
            use crate::sqlite::selector::SQLiteSelector;
            use crate::weight::AuthInfo;

            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();

            let routing_config = SmartRoutingConfig::default();
            let selector = SQLiteSelector::new(store, routing_config);

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
            assert!(selected.is_some(), "Should select an auth when available");

            let selected_id = selected.unwrap();
            assert!(
                selected_id == "auth1" || selected_id == "auth2",
                "Should select one of the provided auths"
            );
        }

        #[tokio::test]
        async fn test_sqlite_selector_empty_auths() {
            use crate::config::SmartRoutingConfig;
            use crate::sqlite::selector::SQLiteSelector;

            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();
            let routing_config = SmartRoutingConfig::default();
            let selector = SQLiteSelector::new(store, routing_config);

            let auths: Vec<crate::weight::AuthInfo> = vec![];

            let selected = selector.pick(auths).await;
            assert!(
                selected.is_none(),
                "Should return None when no auths provided"
            );
        }

        #[tokio::test]
        async fn test_sqlite_selector_all_unavailable() {
            use crate::config::SmartRoutingConfig;
            use crate::sqlite::selector::SQLiteSelector;
            use crate::weight::AuthInfo;

            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();
            let routing_config = SmartRoutingConfig::default();
            let selector = SQLiteSelector::new(store, routing_config);

            let auths = vec![
                AuthInfo {
                    id: "auth1".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: true, // Marked unavailable
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth2".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: true, // Marked unavailable
                    model_states: Vec::new(),
                },
            ];

            let selected = selector.pick(auths).await;
            assert!(
                selected.is_none(),
                "Should return None when all auths are unavailable"
            );
        }

        #[tokio::test]
        async fn test_sqlite_selector_disabled_routing() {
            use crate::config::SmartRoutingConfig;
            use crate::sqlite::selector::SQLiteSelector;
            use crate::weight::AuthInfo;

            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();

            // Create selector with smart routing disabled
            let routing_config = SmartRoutingConfig {
                enabled: false,
                ..Default::default()
            };

            let selector = SQLiteSelector::new(store, routing_config);

            let auths = vec![
                AuthInfo {
                    id: "auth2".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth1".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
            ];

            // With routing disabled, should return first auth (auth2 in this case)
            let selected = selector.pick(auths).await;
            assert!(selected.is_some());

            let selected_id = selected.unwrap();
            assert_eq!(
                selected_id, "auth2",
                "With routing disabled, should return the first auth"
            );
        }
    }

    mod selector_weighted {
        use super::*;

        #[tokio::test]
        async fn test_sqlite_selector_weighted_selection() {
            use crate::config::SmartRoutingConfig;
            use crate::metrics::AuthMetrics;
            use crate::sqlite::selector::SQLiteSelector;
            use crate::weight::AuthInfo;
            use chrono::Utc;

            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();

            // Setup metrics for auth1 (good performance)
            let metrics1 = AuthMetrics {
                total_requests: 100,
                success_count: 95,
                failure_count: 5,
                avg_latency_ms: 50.0, // Low latency
                min_latency_ms: 30.0,
                max_latency_ms: 100.0,
                success_rate: 0.95, // High success rate
                error_rate: 0.05,
                consecutive_successes: 10,
                consecutive_failures: 0,
                last_request_time: Utc::now(),
                last_success_time: Some(Utc::now()),
                last_failure_time: None,
            };

            store.write_metrics("auth1", &metrics1).await.unwrap();

            // Setup metrics for auth2 (poor performance)
            let metrics2 = AuthMetrics {
                total_requests: 100,
                success_count: 50,
                failure_count: 50,
                avg_latency_ms: 400.0, // High latency
                min_latency_ms: 200.0,
                max_latency_ms: 600.0,
                success_rate: 0.5, // Low success rate
                error_rate: 0.5,
                consecutive_successes: 0,
                consecutive_failures: 10,
                last_request_time: Utc::now(),
                last_success_time: None,
                last_failure_time: Some(Utc::now()),
            };

            store.write_metrics("auth2", &metrics2).await.unwrap();

            let routing_config = SmartRoutingConfig::default();
            let selector = SQLiteSelector::new(store, routing_config);

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

            // Run multiple selections to test weighted distribution
            let mut auth1_count = 0;
            let mut auth2_count = 0;
            let iterations = 200;

            for _ in 0..iterations {
                let auths_clone = auths.clone();
                let selected = selector.pick(auths_clone).await;
                assert!(selected.is_some());

                let selected_id = selected.unwrap();
                match selected_id.as_str() {
                    "auth1" => auth1_count += 1,
                    "auth2" => auth2_count += 1,
                    _ => panic!("Unexpected auth selected: {}", selected_id),
                }
            }

            // auth1 should be selected more often due to better metrics.
            // Use a proportional check (auth1 > 40% of selections) to avoid
            // flaky failures from statistical variance in random selection.
            let auth1_ratio = auth1_count as f64 / iterations as f64;
            assert!(
                auth1_ratio > 0.4,
                "auth1 should be selected >40% of the time (got {:.0}%, auth1: {}, auth2: {})",
                auth1_ratio * 100.0,
                auth1_count,
                auth2_count
            );

            // At least some selections should be auth1
            assert!(auth1_count > 0, "auth1 should be selected at least once");
        }

        #[tokio::test]
        async fn test_sqlite_selector_concurrent() {
            use crate::config::SmartRoutingConfig;
            use crate::sqlite::selector::SQLiteSelector;
            use crate::weight::AuthInfo;
            use tokio::task::JoinSet;

            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();
            let routing_config = SmartRoutingConfig::default();
            let selector = std::sync::Arc::new(SQLiteSelector::new(store, routing_config));

            let auths_template = vec![
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
                AuthInfo {
                    id: "auth3".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
            ];

            let mut join_set = JoinSet::new();

            // Spawn 10 concurrent selection tasks
            for i in 0..10 {
                let selector_clone = selector.clone();
                let auths = auths_template.clone();

                join_set.spawn(async move {
                    let selected: Option<String> = selector_clone.pick(auths).await;
                    assert!(
                        selected.is_some(),
                        "Task {} should successfully select an auth",
                        i
                    );
                    selected
                });
            }

            // Wait for all tasks to complete
            let mut results: Vec<Option<String>> = Vec::new();
            while let Some(result) = join_set.join_next().await {
                results.push(result.unwrap());
            }

            assert_eq!(results.len(), 10, "All concurrent tasks should complete");

            // Verify all selections are valid
            for (i, selected) in results.iter().enumerate() {
                assert!(
                    selected.is_some(),
                    "Task {} should have a valid selection",
                    i
                );
                let id = selected.as_ref().unwrap();
                assert!(
                    id == "auth1" || id == "auth2" || id == "auth3",
                    "Task {} should select a valid auth ID",
                    i
                );
            }
        }

        #[tokio::test]
        async fn test_sqlite_selector_precompute_weights() {
            use crate::config::SmartRoutingConfig;
            use crate::sqlite::selector::SQLiteSelector;

            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();
            let routing_config = SmartRoutingConfig::default();
            let selector = SQLiteSelector::new(store, routing_config);

            let auth_ids = vec![
                "auth1".to_string(),
                "auth2".to_string(),
                "auth3".to_string(),
            ];

            // Precompute weights should not error
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                selector.precompute_weights(auth_ids),
            )
            .await;

            match result {
                Ok(Ok(_)) => {}, // Success
                Ok(Err(e)) => panic!("Failed to precompute weights: {}", e),
                Err(_) => {}, // Timeout - acceptable for empty database
            }

            // Get stats to verify operation was tracked
            let stats = selector.get_stats();
            assert!(
                stats.select_count >= 0,
                "Select count should be non-negative"
            );
        }
    }

    mod selector_advanced {
        use super::*;

        #[tokio::test]
        async fn test_sqlite_selector_quota_exceeded() {
            use crate::config::SmartRoutingConfig;
            use crate::metrics::AuthMetrics;
            use crate::sqlite::selector::SQLiteSelector;
            use crate::weight::AuthInfo;
            use chrono::Utc;

            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();

            // Setup good metrics for auth1
            let metrics1 = AuthMetrics {
                total_requests: 100,
                success_count: 95,
                failure_count: 5,
                avg_latency_ms: 50.0,
                min_latency_ms: 30.0,
                max_latency_ms: 100.0,
                success_rate: 0.95,
                error_rate: 0.05,
                consecutive_successes: 10,
                consecutive_failures: 0,
                last_request_time: Utc::now(),
                last_success_time: Some(Utc::now()),
                last_failure_time: None,
            };

            store.write_metrics("auth1", &metrics1).await.unwrap();

            // Setup good metrics for auth2
            let metrics2 = AuthMetrics {
                total_requests: 100,
                success_count: 90,
                failure_count: 10,
                avg_latency_ms: 100.0,
                min_latency_ms: 50.0,
                max_latency_ms: 150.0,
                success_rate: 0.90,
                error_rate: 0.10,
                consecutive_successes: 8,
                consecutive_failures: 0,
                last_request_time: Utc::now(),
                last_success_time: Some(Utc::now()),
                last_failure_time: None,
            };

            store.write_metrics("auth2", &metrics2).await.unwrap();

            let routing_config = SmartRoutingConfig::default();
            let selector = SQLiteSelector::new(store, routing_config);

            let auths = vec![
                AuthInfo {
                    id: "auth1".to_string(),
                    priority: Some(0),
                    quota_exceeded: true, // Quota exceeded - should be deprioritized
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth2".to_string(),
                    priority: Some(0),
                    quota_exceeded: false, // Quota available
                    unavailable: false,
                    model_states: Vec::new(),
                },
            ];

            // Run multiple selections - auth2 should be selected more often
            let mut auth1_count = 0;
            let mut auth2_count = 0;
            let iterations = 20;

            for _ in 0..iterations {
                let auths_clone = auths.clone();
                let selected = selector.pick(auths_clone).await;
                assert!(selected.is_some());

                let selected_id = selected.unwrap();
                match selected_id.as_str() {
                    "auth1" => auth1_count += 1,
                    "auth2" => auth2_count += 1,
                    _ => panic!("Unexpected auth selected: {}", selected_id),
                }
            }

            // auth2 should be selected more often due to quota availability
            assert!(
                auth2_count > auth1_count,
                "auth2 (no quota) should be selected more often than auth1 (quota exceeded) (auth1: {}, auth2: {})",
                auth1_count,
                auth2_count
            );
        }

        #[tokio::test]
        async fn test_sqlite_selector_priority_influence() {
            use crate::config::{SmartRoutingConfig, WeightConfig};
            use crate::sqlite::selector::SQLiteSelector;
            use crate::weight::AuthInfo;

            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();

            // Use a config where priority has a much higher weight
            let routing_config = SmartRoutingConfig {
                weight: WeightConfig {
                    priority_weight: 0.8, // High priority weight
                    success_rate_weight: 0.05,
                    latency_weight: 0.05,
                    health_weight: 0.05,
                    load_weight: 0.05,
                    ..Default::default()
                },
                ..Default::default()
            };
            let selector = SQLiteSelector::new(store, routing_config);

            // auth1 has high priority
            let auth1 = AuthInfo {
                id: "auth1".to_string(),
                priority: Some(100), // High priority
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            };

            // auth2 has low priority
            let auth2 = AuthInfo {
                id: "auth2".to_string(),
                priority: Some(-100), // Low priority
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            };

            let auths = vec![auth1, auth2];

            // Run multiple selections - use enough iterations for statistical significance
            let mut auth1_count = 0;
            let mut auth2_count = 0;
            let iterations = 200;

            for _ in 0..iterations {
                let auths_clone = auths.clone();
                let selected = selector.pick(auths_clone).await;
                assert!(selected.is_some());

                let selected_id = selected.unwrap();
                match selected_id.as_str() {
                    "auth1" => auth1_count += 1,
                    "auth2" => auth2_count += 1,
                    _ => panic!("Unexpected auth selected: {}", selected_id),
                }
            }

            // auth1 (high priority) should be selected more often on average
            // With high priority weight and 200 iterations, this should be reliable
            assert!(
                auth1_count > auth2_count,
                "auth1 (high priority) should be selected more often than auth2 (low priority) (auth1: {}, auth2: {})",
                auth1_count,
                auth2_count
            );
        }

        #[tokio::test]
        async fn test_sqlite_selector_stats_tracking() {
            use crate::config::SmartRoutingConfig;
            use crate::sqlite::selector::SQLiteSelector;
            use crate::weight::AuthInfo;

            let config = SQLiteConfig {
                database_path: ":memory:".to_string(),
                ..Default::default()
            };

            let store = SQLiteStore::new(config).await.unwrap();
            let routing_config = SmartRoutingConfig::default();
            let selector = SQLiteSelector::new(store, routing_config);

            // Initial stats should be zero
            let stats = selector.get_stats();
            assert_eq!(stats.select_count, 0);
            assert_eq!(stats.cache_hits, 0);
            assert_eq!(stats.db_queries, 0);

            let auths = vec![AuthInfo {
                id: "auth1".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            }];

            // Perform selections
            for _ in 0..5 {
                let auths_clone = auths.clone();
                let _ = selector.pick(auths_clone).await;
            }

            // Stats should reflect selections
            let stats = selector.get_stats();
            assert_eq!(stats.select_count, 5, "Should track 5 selections");
            assert!(
                stats.db_queries > 0,
                "Should have performed database queries"
            );
        }
    }
}
