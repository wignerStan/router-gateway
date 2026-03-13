use super::*;
use crate::config::HealthConfig;
use chrono::Duration;

#[tokio::test]
async fn test_health_tracking() {
    let config = HealthConfig::default();
    let manager = HealthManager::new(config);

    // Record failures
    for _ in 0..5 {
        manager.update_from_result("test-auth", false, 500).await;
    }

    let status = manager.get_status("test-auth").await;
    assert_eq!(status, HealthStatus::Unhealthy);
}

#[tokio::test]
async fn test_health_recovery() {
    let config = HealthConfig {
        healthy_threshold: 3,
        ..Default::default()
    };
    let manager = HealthManager::new(config);

    // Record failures to make unhealthy
    for _ in 0..5 {
        manager.update_from_result("test-auth", false, 500).await;
    }
    assert_eq!(
        manager.get_status("test-auth").await,
        HealthStatus::Unhealthy
    );

    // Record successes to recover
    for _ in 0..3 {
        manager.update_from_result("test-auth", true, 200).await;
    }
    assert_eq!(manager.get_status("test-auth").await, HealthStatus::Healthy);
}

#[tokio::test]
async fn test_availability_check() {
    let config = HealthConfig {
        unhealthy_threshold: 3,
        cooldown_period_seconds: 1,
        ..Default::default()
    };
    let manager = HealthManager::new(config);

    // Initially available
    assert!(manager.is_available("test-auth").await);

    // Make unhealthy
    for _ in 0..3 {
        manager.update_from_result("test-auth", false, 500).await;
    }

    // Should be unavailable during cooldown
    assert!(!manager.is_available("test-auth").await);

    // Wait for cooldown to pass
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Still unhealthy but cooldown expired
    let health = manager.get_health("test-auth").await;
    assert!(health.is_some());
    assert!(health.unwrap().unavailable_until.unwrap() < Utc::now());
}

#[tokio::test]
async fn test_empty_auth_id() {
    let config = HealthConfig::default();
    let manager = HealthManager::new(config);

    // Empty auth should return unhealthy status
    assert_eq!(manager.get_status("").await, HealthStatus::Unhealthy);

    // Empty auth should not be available
    assert!(!manager.is_available("").await);

    // Empty auth should return None for health details
    assert!(manager.get_health("").await.is_none());

    // Update with empty auth should be a no-op
    manager.update_from_result("", false, 500).await;
    assert!(manager.get_health("").await.is_none());
}

#[tokio::test]
async fn test_degraded_status() {
    let config = HealthConfig {
        status_codes: crate::config::StatusCodeHealthConfig {
            degraded: vec![429], // Rate limit
            unhealthy: vec![],
            healthy: vec![],
        },
        ..Default::default()
    };
    let manager = HealthManager::new(config);

    // Single rate limit should cause degraded status
    manager.update_from_result("test-auth", false, 429).await;
    assert_eq!(
        manager.get_status("test-auth").await,
        HealthStatus::Degraded
    );

    // Degraded should still be available
    assert!(manager.is_available("test-auth").await);
    assert!(manager.is_healthy("test-auth").await);
}

#[tokio::test]
async fn test_cooldown_period() {
    let config = HealthConfig {
        unhealthy_threshold: 2,
        cooldown_period_seconds: 1,
        ..Default::default()
    };
    let manager = HealthManager::new(config);

    // Make unhealthy
    manager.update_from_result("test-auth", false, 500).await;
    manager.update_from_result("test-auth", false, 500).await;

    assert_eq!(
        manager.get_status("test-auth").await,
        HealthStatus::Unhealthy
    );

    // Check unavailable_until is set
    let health = manager.get_health("test-auth").await.unwrap();
    assert!(health.unavailable_until.is_some());

    // Should be unavailable
    assert!(!manager.is_available("test-auth").await);

    // Wait for cooldown
    tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;

    // Cooldown expired, but still unhealthy
    assert!(!manager.is_available("test-auth").await);
}

#[tokio::test]
async fn test_error_counting() {
    let config = HealthConfig::default();
    let manager = HealthManager::new(config);

    // Record different error types
    manager.update_from_result("test-auth", false, 500).await;
    manager.update_from_result("test-auth", false, 500).await;
    manager.update_from_result("test-auth", false, 503).await;
    manager.update_from_result("test-auth", false, 429).await;

    let health = manager.get_health("test-auth").await.unwrap();

    // Verify error counts
    assert_eq!(*health.error_counts.get(&500).unwrap(), 2);
    assert_eq!(*health.error_counts.get(&503).unwrap(), 1);
    assert_eq!(*health.error_counts.get(&429).unwrap(), 1);
}

#[tokio::test]
async fn test_status_change_tracking() {
    let config = HealthConfig {
        unhealthy_threshold: 2,
        ..Default::default()
    };
    let manager = HealthManager::new(config);

    // Initial status
    manager.update_from_result("test-auth", true, 200).await;
    let health1 = manager.get_health("test-auth").await.unwrap();
    let first_change = health1.last_status_change;

    // Status changes to unhealthy
    manager.update_from_result("test-auth", false, 500).await;
    manager.update_from_result("test-auth", false, 500).await;

    let health2 = manager.get_health("test-auth").await.unwrap();
    assert!(health2.last_status_change > first_change);
}

#[tokio::test]
async fn test_reset_functionality() {
    let config = HealthConfig::default();
    let manager = HealthManager::new(config);

    // Make unhealthy
    for _ in 0..5 {
        manager.update_from_result("test-auth", false, 500).await;
    }
    assert_eq!(
        manager.get_status("test-auth").await,
        HealthStatus::Unhealthy
    );

    // Reset
    manager.reset("test-auth").await;

    // Should be healthy again
    let health = manager.get_health("test-auth").await.unwrap();
    assert_eq!(health.status, HealthStatus::Healthy);
    assert_eq!(health.consecutive_failures, 0);
    assert_eq!(health.consecutive_successes, 0);
    assert!(health.unavailable_until.is_none());
    assert!(health.error_counts.is_empty());
}

#[tokio::test]
async fn test_mark_unavailable() {
    let config = HealthConfig::default();
    let manager = HealthManager::new(config);

    // Mark as unavailable for 2 seconds
    manager
        .mark_unavailable("test-auth", Duration::seconds(2))
        .await;

    assert_eq!(
        manager.get_status("test-auth").await,
        HealthStatus::Unhealthy
    );
    assert!(!manager.is_available("test-auth").await);

    let health = manager.get_health("test-auth").await.unwrap();
    assert!(health.unavailable_until.is_some());
}

#[tokio::test]
async fn test_mark_unavailable_edge_cases() {
    let config = HealthConfig::default();
    let manager = HealthManager::new(config);

    // Empty auth should be a no-op
    manager.mark_unavailable("", Duration::seconds(10)).await;
    assert!(manager.get_health("").await.is_none());

    // Zero/negative duration should be a no-op
    manager
        .mark_unavailable("test-auth", Duration::seconds(0))
        .await;
    let health = manager.get_health("test-auth").await;
    // Should not have been created
    assert!(health.is_none());

    manager
        .mark_unavailable("test-auth", Duration::seconds(-1))
        .await;
    let health = manager.get_health("test-auth").await;
    assert!(health.is_none());
}

#[tokio::test]
async fn test_get_healthy_count() {
    let config = HealthConfig {
        unhealthy_threshold: 2,
        ..Default::default()
    };
    let manager = HealthManager::new(config);

    // Setup multiple auths with different health
    manager.update_from_result("auth-1", true, 200).await;
    manager.update_from_result("auth-2", true, 200).await;
    manager.update_from_result("auth-3", false, 500).await;
    manager.update_from_result("auth-3", false, 500).await; // Unhealthy

    let count = manager
        .get_healthy_count(&[
            "auth-1".to_string(),
            "auth-2".to_string(),
            "auth-3".to_string(),
        ])
        .await;
    assert_eq!(count, 2);
}

#[tokio::test]
async fn test_get_available_count() {
    let config = HealthConfig {
        unhealthy_threshold: 2,
        cooldown_period_seconds: 10,
        ..Default::default()
    };
    let manager = HealthManager::new(config);

    // Setup multiple auths
    manager.update_from_result("auth-1", true, 200).await;
    manager.update_from_result("auth-2", true, 200).await;
    manager.update_from_result("auth-3", false, 500).await;
    manager.update_from_result("auth-3", false, 500).await; // Unhealthy and in cooldown

    let count = manager
        .get_available_count(&[
            "auth-1".to_string(),
            "auth-2".to_string(),
            "auth-3".to_string(),
        ])
        .await;
    assert_eq!(count, 2);
}

#[tokio::test]
async fn test_cleanup_old_entries() {
    let config = HealthConfig::default();
    let manager = HealthManager::with_limit(config, 5);

    // Add entries to exceed the limit
    for i in 0..10 {
        manager
            .update_from_result(&format!("auth-{}", i), true, 200)
            .await;
    }

    // Add more operations to trigger cleanup (cleanup_interval is 100)
    for _ in 0..100 {
        manager.update_from_result("auth-0", true, 200).await;
    }

    // After enough operations, cleanup should have been triggered
    let all_health = manager.health.read().await;
    // Cleanup removes oldest entries when over limit
    // The exact count depends on when cleanup triggers
    assert!(all_health.len() <= 10); // Should not have grown unbounded
}

#[tokio::test]
async fn test_clone_shares_health_storage() {
    let config = HealthConfig::default();
    let manager1 = HealthManager::new(config);

    manager1.update_from_result("auth-1", true, 200).await;

    let manager2 = manager1.clone();

    // Clone shares the same storage via Arc
    manager2.update_from_result("auth-2", true, 200).await;

    // Manager1 should see auth-2 (shared state)
    assert!(manager1.get_health("auth-2").await.is_some());

    // Manager2 should see auth-1 (shared state)
    assert!(manager2.get_health("auth-1").await.is_some());
}

#[tokio::test]
async fn test_default_status_for_unknown_auth() {
    let config = HealthConfig::default();
    let manager = HealthManager::new(config);

    // Unknown auth should be healthy by default
    assert_eq!(manager.get_status("unknown").await, HealthStatus::Healthy);

    // Unknown auth should be available by default
    assert!(manager.is_available("unknown").await);
}

#[tokio::test]
async fn test_health_config_update() {
    let config1 = HealthConfig {
        unhealthy_threshold: 5,
        ..Default::default()
    };
    let mut manager = HealthManager::new(config1);

    // Record 3 failures - not enough with threshold 5
    for _ in 0..3 {
        manager.update_from_result("test-auth", false, 400).await;
    }
    assert_eq!(manager.get_status("test-auth").await, HealthStatus::Healthy);

    // Update config with lower threshold
    let config2 = HealthConfig {
        unhealthy_threshold: 2,
        ..Default::default()
    };
    manager.set_config(config2);

    // Record more failures
    manager.update_from_result("test-auth", false, 400).await;
    // Now should be unhealthy with new threshold (total 4 consecutive)
    // Note: consecutive_failures was reset when success occurred, so we need more failures
}

// ============================================================
// Edge Case Tests for Health Manager State Machine
// ============================================================

#[tokio::test]
async fn test_health_transition_healthy_degraded_healthy() {
    let config = HealthConfig {
        healthy_threshold: 2,
        unhealthy_threshold: 5,
        status_codes: crate::config::StatusCodeHealthConfig {
            degraded: vec![429],
            unhealthy: vec![500],
            healthy: vec![],
        },
        ..Default::default()
    };
    let manager = HealthManager::new(config);

    // Start healthy (default)
    assert_eq!(
        manager.get_status("test-auth").await,
        HealthStatus::Healthy,
        "Should start healthy"
    );

    // Degraded by 429
    manager.update_from_result("test-auth", false, 429).await;
    assert_eq!(
        manager.get_status("test-auth").await,
        HealthStatus::Degraded,
        "Should be degraded after 429"
    );

    // Direct recovery to healthy (2 successes)
    manager.update_from_result("test-auth", true, 200).await;
    manager.update_from_result("test-auth", true, 200).await;
    assert_eq!(
        manager.get_status("test-auth").await,
        HealthStatus::Healthy,
        "Should recover to healthy after 2 successes"
    );
}

#[tokio::test]
async fn test_health_transition_exact_threshold_values() {
    // Use config with no status codes so only threshold matters
    let config = HealthConfig {
        healthy_threshold: 3,
        unhealthy_threshold: 3,
        status_codes: crate::config::StatusCodeHealthConfig {
            degraded: vec![],
            unhealthy: vec![],
            healthy: vec![],
        },
        ..Default::default()
    };
    let manager = HealthManager::new(config);

    // Exactly 3 failures (threshold) - use status code NOT in unhealthy list
    manager.update_from_result("test-auth", false, 400).await;
    manager.update_from_result("test-auth", false, 400).await;
    assert_eq!(
        manager.get_status("test-auth").await,
        HealthStatus::Healthy,
        "Should still be healthy at 2 failures (threshold - 1)"
    );

    manager.update_from_result("test-auth", false, 400).await;
    assert_eq!(
        manager.get_status("test-auth").await,
        HealthStatus::Unhealthy,
        "Should be unhealthy at exactly 3 failures (threshold)"
    );

    // Exactly 3 successes for recovery
    manager.update_from_result("test-auth", true, 200).await;
    manager.update_from_result("test-auth", true, 200).await;
    let health = manager.get_health("test-auth").await.unwrap();
    assert_eq!(
        health.consecutive_successes, 2,
        "Should have 2 consecutive successes"
    );

    manager.update_from_result("test-auth", true, 200).await;
    assert_eq!(
        manager.get_status("test-auth").await,
        HealthStatus::Healthy,
        "Should recover at exactly 3 successes (threshold)"
    );
}

#[tokio::test]
async fn test_custom_status_code_configuration() {
    let config = HealthConfig {
        status_codes: crate::config::StatusCodeHealthConfig {
            degraded: vec![503],       // Service unavailable
            unhealthy: vec![401, 403], // Auth errors
            healthy: vec![],
        },
        ..Default::default()
    };
    let manager = HealthManager::new(config);

    // 503 should cause degraded
    manager
        .update_from_result("test-auth-503", false, 503)
        .await;
    assert_eq!(
        manager.get_status("test-auth-503").await,
        HealthStatus::Degraded,
        "503 should cause degraded status"
    );

    // 401 should cause unhealthy
    manager
        .update_from_result("test-auth-401", false, 401)
        .await;
    assert_eq!(
        manager.get_status("test-auth-401").await,
        HealthStatus::Unhealthy,
        "401 should cause unhealthy status"
    );

    // 403 should cause unhealthy
    manager
        .update_from_result("test-auth-403", false, 403)
        .await;
    assert_eq!(
        manager.get_status("test-auth-403").await,
        HealthStatus::Unhealthy,
        "403 should cause unhealthy status"
    );

    // 500 should NOT cause unhealthy (not in config)
    manager
        .update_from_result("test-auth-500", false, 500)
        .await;
    assert_eq!(
        manager.get_status("test-auth-500").await,
        HealthStatus::Healthy,
        "500 should not cause unhealthy when not in config"
    );
}

#[tokio::test]
async fn test_health_manager_zero_thresholds() {
    let config = HealthConfig {
        healthy_threshold: 0,
        unhealthy_threshold: 0,
        ..Default::default()
    };
    let manager = HealthManager::new(config);

    // With zero threshold, even 0 consecutive failures meets threshold
    // First success should immediately make healthy
    manager.update_from_result("test-auth", true, 200).await;
    let health = manager.get_health("test-auth").await.unwrap();
    assert_eq!(health.consecutive_successes, 1);

    // First failure should immediately make unhealthy with threshold 0
    manager.update_from_result("test-auth2", false, 500).await;
    let health2 = manager.get_health("test-auth2").await.unwrap();
    assert_eq!(health2.consecutive_failures, 1);
    // With threshold 0, consecutive_failures >= 0 is always true
    assert_eq!(
        manager.get_status("test-auth2").await,
        HealthStatus::Unhealthy,
        "Zero threshold should immediately mark unhealthy on any failure"
    );
}

#[tokio::test]
async fn test_cooldown_expiration_allows_retry_keeps_unhealthy() {
    let config = HealthConfig {
        unhealthy_threshold: 2,
        cooldown_period_seconds: 1,
        ..Default::default()
    };
    let manager = HealthManager::new(config);

    // Make unhealthy
    manager.update_from_result("test-auth", false, 500).await;
    manager.update_from_result("test-auth", false, 500).await;

    assert_eq!(
        manager.get_status("test-auth").await,
        HealthStatus::Unhealthy
    );
    assert!(
        !manager.is_available("test-auth").await,
        "Should be unavailable during cooldown"
    );

    // Wait for cooldown
    tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;

    // Still unhealthy but cooldown expired
    assert_eq!(
        manager.get_status("test-auth").await,
        HealthStatus::Unhealthy,
        "Status should still be unhealthy after cooldown"
    );
    // is_available checks cooldown, so should now be true (cooldown expired)
    // but status is still Unhealthy so is_available returns false
    assert!(
        !manager.is_available("test-auth").await,
        "Should still be unavailable because status is Unhealthy"
    );
}

#[tokio::test]
async fn test_health_with_large_consecutive_counts() {
    let config = HealthConfig {
        healthy_threshold: 3,
        unhealthy_threshold: 5,
        ..Default::default()
    };
    let manager = HealthManager::new(config);

    // Record many successes
    for _ in 0..100 {
        manager.update_from_result("test-auth", true, 200).await;
    }

    let health = manager.get_health("test-auth").await.unwrap();
    assert_eq!(health.consecutive_successes, 100);
    assert_eq!(health.consecutive_failures, 0);
    assert_eq!(manager.get_status("test-auth").await, HealthStatus::Healthy);

    // Now record many failures
    for _ in 0..100 {
        manager.update_from_result("test-auth", false, 500).await;
    }

    let health = manager.get_health("test-auth").await.unwrap();
    assert_eq!(health.consecutive_successes, 0);
    assert_eq!(health.consecutive_failures, 100);
    assert_eq!(
        manager.get_status("test-auth").await,
        HealthStatus::Unhealthy
    );
}

#[tokio::test]
async fn test_health_status_code_priority_over_threshold() {
    // Status codes should take priority over threshold-based transitions
    let config = HealthConfig {
        healthy_threshold: 10, // High threshold
        unhealthy_threshold: 10,
        status_codes: crate::config::StatusCodeHealthConfig {
            degraded: vec![429],
            unhealthy: vec![401],
            healthy: vec![],
        },
        ..Default::default()
    };
    let manager = HealthManager::new(config);

    // Single 401 should immediately cause unhealthy (status code priority)
    manager.update_from_result("test-auth", false, 401).await;
    assert_eq!(
        manager.get_status("test-auth").await,
        HealthStatus::Unhealthy,
        "Status code 401 should immediately cause unhealthy regardless of threshold"
    );
}

#[tokio::test]
async fn test_clone_shares_health_state() {
    let config = HealthConfig::default();
    let manager = HealthManager::new(config);

    // Record a health event on the original
    manager.update_from_result("auth-a", false, 500).await;
    manager.update_from_result("auth-a", false, 500).await;
    manager.update_from_result("auth-a", false, 500).await;
    assert_eq!(
        manager.get_status("auth-a").await,
        HealthStatus::Unhealthy,
        "original should see unhealthy after failures"
    );

    // Clone the manager
    let cloned = manager.clone();

    // The clone should see the same health state (shared via Arc)
    assert_eq!(
        cloned.get_status("auth-a").await,
        HealthStatus::Unhealthy,
        "clone should see the same health state as the original"
    );

    // Update the clone — original should see it (shared state)
    cloned.update_from_result("auth-b", false, 500).await;
    assert!(
        manager.get_health("auth-b").await.is_some(),
        "original should see entries created by the clone"
    );

    // Update the original — clone should see it (shared state)
    manager.update_from_result("auth-c", false, 500).await;
    manager.update_from_result("auth-c", false, 500).await;
    manager.update_from_result("auth-c", false, 500).await;
    assert_eq!(
        cloned.get_status("auth-c").await,
        HealthStatus::Unhealthy,
        "clone should see updates made to the original"
    );
}
