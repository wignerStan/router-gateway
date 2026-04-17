#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::significant_drop_tightening
)]
//! Database resilience tests for `SQLite` store failure paths.
//!
//! Covers: invalid paths, concurrent write contention, concurrent read/write
//! safety, nonexistent auth lookups, and upsert semantics verification.

#[cfg(test)]
mod red_edge {
    use chrono::Utc;
    use gateway::routing::health::{AuthHealth, HealthStatus};
    use gateway::routing::metrics::AuthMetrics;
    use gateway::routing::sqlite::store::SQLiteConfig;
    use gateway::routing::sqlite::store::SQLiteStore;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::task::JoinSet;

    // -- Helper: build a store with in-memory DB (no cache to test raw DB) --

    fn mem_config() -> SQLiteConfig {
        SQLiteConfig {
            database_path: ":memory:".to_string(),
            enable_cache: false,
            ..Default::default()
        }
    }

    fn sample_metrics(
        total: i64,
        success: i64,
        failure: i64,
        avg_latency: f64,
        success_rate: f64,
    ) -> AuthMetrics {
        AuthMetrics {
            total_requests: total,
            success_count: success,
            failure_count: failure,
            avg_latency_ms: avg_latency,
            min_latency_ms: 30.0,
            max_latency_ms: 200.0,
            success_rate,
            error_rate: 1.0 - success_rate,
            consecutive_successes: success as i32,
            consecutive_failures: failure as i32,
            last_request_time: Utc::now(),
            last_success_time: Some(Utc::now()),
            last_failure_time: if failure > 0 { Some(Utc::now()) } else { None },
        }
    }

    fn sample_health(status: HealthStatus, successes: i32, failures: i32) -> AuthHealth {
        AuthHealth {
            status,
            consecutive_successes: successes,
            consecutive_failures: failures,
            last_status_change: Utc::now(),
            last_check_time: Utc::now(),
            unavailable_until: None,
            error_counts: HashMap::new(),
        }
    }

    // -- Test 1: Invalid database path (deeply nested nonexistent dir) --

    #[tokio::test]
    async fn test_invalid_database_path_errors() {
        // Use a deeply nested nonexistent directory path
        let config = SQLiteConfig {
            database_path: "/tmp/nonexistent_dir_a/b/c/d/e/test.db".to_string(),
            ..Default::default()
        };

        let result = SQLiteStore::new(config).await;
        assert!(
            result.is_err(),
            "Creating a store with a deeply nested nonexistent path should fail"
        );
    }

    // -- Test 2: Concurrent writes with no data loss (10 tasks x 50 writes) --

    #[tokio::test]
    async fn test_concurrent_writes_no_data_loss() {
        // Use a file-backed DB so we can have multiple connections (WAL mode).
        // In-memory SQLite with sqlx limits to 1 connection, so file-backed is
        // required for meaningful concurrency testing.
        let dir = tempfile::tempdir().expect("tempdir should be created");
        let db_path = dir.path().join("concurrent_writes.db");
        let config = SQLiteConfig {
            database_path: db_path.to_string_lossy().to_string(),
            enable_cache: false,
            ..Default::default()
        };

        let store = Arc::new(
            SQLiteStore::new(config)
                .await
                .expect("store creation should succeed"),
        );

        let tasks = 10;
        let writes_per_task = 50;
        let mut join_set = JoinSet::new();

        for i in 0..tasks {
            let store_clone = Arc::clone(&store);
            join_set.spawn(async move {
                let auth_id = format!("concurrent-auth-{i}");
                for j in 0..writes_per_task {
                    let metrics = sample_metrics(
                        i64::from(j + 1),
                        i64::from(j),
                        i64::from(j != 0),
                        50.0 + f64::from(j),
                        0.9,
                    );
                    store_clone
                        .write_metrics(&auth_id, &metrics)
                        .await
                        .expect("write_metrics should succeed under contention");
                }
            });
        }

        // All tasks should complete without error
        while let Some(result) = join_set.join_next().await {
            result.expect("task should not panic");
        }

        // Verify each of the 10 auths has exactly 50 writes (upsert semantics
        // mean only the last value per auth is visible).
        for i in 0..tasks {
            let auth_id = format!("concurrent-auth-{i}");
            let loaded = store
                .load_metrics(&auth_id)
                .await
                .expect("load_metrics should succeed")
                .expect("metrics should exist after concurrent writes");

            // Each task wrote 50 entries for the same auth_id using upsert.
            // The final write (j=49) should be the one persisted.
            assert_eq!(
                loaded.total_requests, 50,
                "auth {auth_id} should have the last written value (50 total_requests)"
            );
        }
    }

    // -- Test 3: Concurrent read/write no panic (5 readers + 5 writers) --

    #[tokio::test]
    async fn test_concurrent_read_write_no_panic() {
        let dir = tempfile::tempdir().expect("tempdir should be created");
        let db_path = dir.path().join("rw_mix.db");
        let config = SQLiteConfig {
            database_path: db_path.to_string_lossy().to_string(),
            enable_cache: false,
            ..Default::default()
        };

        let store = Arc::new(
            SQLiteStore::new(config)
                .await
                .expect("store creation should succeed"),
        );

        // Seed initial data
        for i in 0..5 {
            let metrics = sample_metrics(10, 9, 1, 100.0, 0.9);
            store
                .write_metrics(&format!("rw-auth-{i}"), &metrics)
                .await
                .expect("seed write should succeed");

            let health = sample_health(HealthStatus::Healthy, 5, 0);
            store
                .write_health(&format!("rw-auth-{i}"), &health)
                .await
                .expect("seed health write should succeed");
        }

        let iterations = 50;
        let mut join_set = JoinSet::new();

        // 5 writers: write metrics and health to rotating auth IDs
        for i in 0..5 {
            let store_clone = Arc::clone(&store);
            join_set.spawn(async move {
                for j in 0..iterations {
                    let auth_id = format!("rw-auth-{}", (i + j) % 5);
                    let metrics = sample_metrics(
                        i64::from(i * iterations + j + 1),
                        i64::from(i * iterations + j),
                        1,
                        50.0 + f64::from(j),
                        0.85,
                    );
                    store_clone
                        .write_metrics(&auth_id, &metrics)
                        .await
                        .expect("concurrent write_metrics should not fail");

                    let health = if j % 3 == 0 {
                        sample_health(HealthStatus::Degraded, 0, j)
                    } else {
                        sample_health(HealthStatus::Healthy, j, 0)
                    };
                    store_clone
                        .write_health(&auth_id, &health)
                        .await
                        .expect("concurrent write_health should not fail");
                }
            });
        }

        // 5 readers: read metrics and health from rotating auth IDs
        for i in 0..5 {
            let store_clone = Arc::clone(&store);
            join_set.spawn(async move {
                for j in 0..iterations {
                    let auth_id = format!("rw-auth-{}", (i + j) % 5);
                    // Reads should never panic or error
                    let metrics_result = store_clone.load_metrics(&auth_id).await;
                    assert!(
                        metrics_result.is_ok(),
                        "load_metrics should not error under contention"
                    );

                    let health_result = store_clone.load_health(&auth_id).await;
                    assert!(
                        health_result.is_ok(),
                        "load_health should not error under contention"
                    );

                    // load_all variants should also be safe
                    let all_metrics = store_clone.load_all_metrics().await;
                    assert!(
                        all_metrics.is_ok(),
                        "load_all_metrics should not error under contention"
                    );

                    let all_health = store_clone.load_all_health().await;
                    assert!(
                        all_health.is_ok(),
                        "load_all_health should not error under contention"
                    );

                    // Suppress unused warnings
                    let _ = metrics_result;
                    let _ = health_result;
                    let _ = all_metrics;
                    let _ = all_health;
                }
            });
        }

        // All tasks should complete without panic
        while let Some(result) = join_set.join_next().await {
            result.expect("concurrent read/write task should not panic");
        }
    }

    // -- Test 4: Load metrics for nonexistent auth returns None --

    #[tokio::test]
    async fn test_load_metrics_nonexistent_returns_none() {
        let store = SQLiteStore::new(mem_config())
            .await
            .expect("store creation should succeed");

        let result = store.load_metrics("auth-that-does-not-exist").await;
        assert!(
            result.is_ok(),
            "load_metrics should not error for nonexistent auth"
        );
        assert!(
            result.unwrap().is_none(),
            "load_metrics should return None for nonexistent auth"
        );
    }

    // -- Test 5: Load health for nonexistent auth returns None --

    #[tokio::test]
    async fn test_load_health_nonexistent_returns_none() {
        let store = SQLiteStore::new(mem_config())
            .await
            .expect("store creation should succeed");

        let result = store.load_health("auth-that-does-not-exist").await;
        assert!(
            result.is_ok(),
            "load_health should not error for nonexistent auth"
        );
        assert!(
            result.unwrap().is_none(),
            "load_health should return None for nonexistent auth"
        );
    }

    // -- Test 6: Upsert replaces existing metrics --

    #[tokio::test]
    async fn test_upsert_replaces_existing_metrics() {
        let store = SQLiteStore::new(mem_config())
            .await
            .expect("store creation should succeed");

        let auth_id = "upsert-target";

        // Write initial metrics
        let v1 = sample_metrics(10, 9, 1, 100.0, 0.9);
        store
            .write_metrics(auth_id, &v1)
            .await
            .expect("first write should succeed");

        // Verify initial write
        let loaded_v1 = store
            .load_metrics(auth_id)
            .await
            .expect("load should succeed")
            .expect("metrics should exist");
        assert_eq!(loaded_v1.total_requests, 10);
        assert_eq!(loaded_v1.success_count, 9);
        assert!((loaded_v1.avg_latency_ms - 100.0).abs() < 0.01);

        // Write replacement metrics (upsert)
        let v2 = sample_metrics(999, 800, 199, 42.5, 0.801);
        store
            .write_metrics(auth_id, &v2)
            .await
            .expect("upsert write should succeed");

        // Verify replacement
        let loaded_v2 = store
            .load_metrics(auth_id)
            .await
            .expect("load after upsert should succeed")
            .expect("metrics should exist after upsert");

        assert_eq!(
            loaded_v2.total_requests, 999,
            "upsert should replace total_requests"
        );
        assert_eq!(
            loaded_v2.success_count, 800,
            "upsert should replace success_count"
        );
        assert_eq!(
            loaded_v2.failure_count, 199,
            "upsert should replace failure_count"
        );
        assert!(
            (loaded_v2.avg_latency_ms - 42.5).abs() < 0.01,
            "upsert should replace avg_latency_ms"
        );

        // Only one row should exist (not duplicated)
        let all = store
            .load_all_metrics()
            .await
            .expect("load_all should succeed");
        assert_eq!(all.len(), 1, "upsert should not duplicate rows");
    }

    // -- Test 7: Upsert replaces existing health --

    #[tokio::test]
    async fn test_upsert_replaces_existing_health() {
        let store = SQLiteStore::new(mem_config())
            .await
            .expect("store creation should succeed");

        let auth_id = "upsert-health-target";

        // Write initial health
        let v1 = sample_health(HealthStatus::Healthy, 10, 0);
        store
            .write_health(auth_id, &v1)
            .await
            .expect("first write should succeed");

        // Write replacement health (upsert)
        let mut v2_errors = HashMap::new();
        v2_errors.insert(500, 5);
        v2_errors.insert(503, 2);
        let v2 = AuthHealth {
            status: HealthStatus::Unhealthy,
            consecutive_successes: 0,
            consecutive_failures: 7,
            last_status_change: Utc::now(),
            last_check_time: Utc::now(),
            unavailable_until: Some(Utc::now() + chrono::Duration::minutes(10)),
            error_counts: v2_errors,
        };
        store
            .write_health(auth_id, &v2)
            .await
            .expect("upsert write should succeed");

        // Verify replacement
        let loaded = store
            .load_health(auth_id)
            .await
            .expect("load after upsert should succeed")
            .expect("health should exist after upsert");

        assert_eq!(loaded.status, HealthStatus::Unhealthy);
        assert_eq!(loaded.consecutive_failures, 7);
        assert_eq!(loaded.consecutive_successes, 0);
        assert_eq!(loaded.error_counts.get(&500), Some(&5));
        assert_eq!(loaded.error_counts.get(&503), Some(&2));
        assert!(loaded.unavailable_until.is_some());

        // Only one row should exist
        let all = store
            .load_all_health()
            .await
            .expect("load_all_health should succeed");
        assert_eq!(all.len(), 1, "upsert should not duplicate health rows");
    }
}
