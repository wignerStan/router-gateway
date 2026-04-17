#![allow(
    clippy::unreadable_literal,
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used
)]
// Integration tests for health management
//
// Unit-level integration tests covering health state transitions,
// cooldown periods, and credential availability.
// Behavioral BDD coverage lives in tests/bdd/health.rs (cucumber).

#[cfg(test)]
mod health {
    use gateway::routing::config::{HealthConfig, StatusCodeHealthConfig};
    use gateway::routing::health::{HealthManager, HealthStatus};
    use rstest::rstest;
    use std::time::Duration;

    #[rstest]
    #[case::rate_limit(429, HealthStatus::Degraded)]
    #[case::service_unavailable(503, HealthStatus::Unhealthy)]
    #[tokio::test]
    async fn test_single_failure_triggers_status(
        #[case] status_code: i32,
        #[case] expected_status: HealthStatus,
    ) {
        let config = HealthConfig {
            status_codes: StatusCodeHealthConfig {
                degraded: vec![429],
                unhealthy: vec![503],
                healthy: vec![],
            },
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        assert_eq!(manager.get_status("test-auth").await, HealthStatus::Healthy);
        manager
            .update_from_result("test-auth", false, status_code)
            .await;
        assert_eq!(manager.get_status("test-auth").await, expected_status);
    }

    #[rstest]
    #[case::five_failures(5, 500, HealthStatus::Unhealthy)]
    #[tokio::test]
    async fn test_consecutive_failures(
        #[case] failures: u32,
        #[case] status_code: i32,
        #[case] expected_status: HealthStatus,
    ) {
        let config = HealthConfig {
            unhealthy_threshold: 5,
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        assert_eq!(manager.get_status("test-auth").await, HealthStatus::Healthy);
        for _ in 0..failures {
            manager
                .update_from_result("test-auth", false, status_code)
                .await;
        }
        assert_eq!(manager.get_status("test-auth").await, expected_status);
    }

    #[rstest]
    #[case::three_successes(3, 200, HealthStatus::Healthy)]
    #[tokio::test]
    async fn test_success_streak_recovers_degraded(
        #[case] successes: u32,
        #[case] status_code: i32,
        #[case] expected_status: HealthStatus,
    ) {
        let config = HealthConfig {
            healthy_threshold: 3,
            status_codes: StatusCodeHealthConfig {
                degraded: vec![429],
                unhealthy: vec![],
                healthy: vec![],
            },
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        manager.update_from_result("test-auth", false, 429).await;
        assert_eq!(
            manager.get_status("test-auth").await,
            HealthStatus::Degraded
        );

        for _ in 0..successes {
            manager
                .update_from_result("test-auth", true, status_code)
                .await;
        }
        assert_eq!(manager.get_status("test-auth").await, expected_status);
    }

    #[rstest]
    #[case::cooldown_active(10, 0, false)]
    #[case::cooldown_expired(1, 2, false)]
    #[tokio::test]
    async fn test_unhealthy_availability_and_cooldown(
        #[case] cooldown: i64,
        #[case] wait: u64,
        #[case] expected_available: bool,
    ) {
        let config = HealthConfig {
            unhealthy_threshold: 2,
            cooldown_period_seconds: cooldown,
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        for _ in 0..2 {
            manager.update_from_result("test-auth", false, 500).await;
        }

        if wait > 0 {
            tokio::time::sleep(Duration::from_secs(wait)).await;
            assert_eq!(
                manager.get_status("test-auth").await,
                HealthStatus::Unhealthy
            );
        } else {
            assert_eq!(
                manager.get_status("test-auth").await,
                HealthStatus::Unhealthy
            );
            assert_eq!(manager.is_available("test-auth").await, expected_available);
        }
    }

    #[tokio::test]
    async fn test_all_health_state_transitions() {
        let config = HealthConfig {
            status_codes: StatusCodeHealthConfig {
                degraded: vec![429, 503],
                unhealthy: vec![401, 403, 500, 502, 504],
                healthy: vec![],
            },
            unhealthy_threshold: 3,
            healthy_threshold: 3,
            degraded_threshold: 0.5,
            cooldown_period_seconds: 1,
        };
        let manager = HealthManager::new(config);
        let auth_id = "state-transition-test";

        // Start healthy
        assert_eq!(manager.get_status(auth_id).await, HealthStatus::Healthy);

        // Trigger degraded (rate limit)
        manager.update_from_result(auth_id, false, 429).await;
        assert_eq!(manager.get_status(auth_id).await, HealthStatus::Degraded);

        // Trigger unhealthy (3 more failures)
        for _ in 0..3 {
            manager.update_from_result(auth_id, false, 500).await;
        }
        assert_eq!(manager.get_status(auth_id).await, HealthStatus::Unhealthy);

        // Recover to degraded (rate limit response)
        manager.update_from_result(auth_id, false, 429).await;
        assert_eq!(manager.get_status(auth_id).await, HealthStatus::Degraded);

        // Recover to healthy (3 consecutive successes)
        for _ in 0..3 {
            manager.update_from_result(auth_id, true, 200).await;
        }
        assert_eq!(manager.get_status(auth_id).await, HealthStatus::Healthy);
    }
}
