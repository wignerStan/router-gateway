#![allow(clippy::unreadable_literal, missing_docs)]
// Integration tests for health management
//
// Unit-level integration tests covering health state transitions,
// cooldown periods, and credential availability.
// Behavioral BDD coverage lives in tests/bdd/health.rs (cucumber).

#[cfg(test)]
mod health {

    #[tokio::test]
    async fn test_rate_limit_triggers_degraded() {
        use gateway::routing::config::HealthConfig;
        use gateway::routing::health::{HealthManager, HealthStatus};

        let config = HealthConfig {
            status_codes: gateway::routing::config::StatusCodeHealthConfig {
                degraded: vec![429],
                unhealthy: vec![],
                healthy: vec![],
            },
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        assert_eq!(manager.get_status("test-auth").await, HealthStatus::Healthy);
        manager.update_from_result("test-auth", false, 429).await;
        assert_eq!(
            manager.get_status("test-auth").await,
            HealthStatus::Degraded
        );
    }

    #[tokio::test]
    async fn test_consecutive_failures_trigger_unhealthy() {
        use gateway::routing::config::HealthConfig;
        use gateway::routing::health::{HealthManager, HealthStatus};

        let config = HealthConfig {
            unhealthy_threshold: 5,
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        assert_eq!(manager.get_status("test-auth").await, HealthStatus::Healthy);
        for _ in 0..5 {
            manager.update_from_result("test-auth", false, 500).await;
        }
        assert_eq!(
            manager.get_status("test-auth").await,
            HealthStatus::Unhealthy
        );
    }

    #[tokio::test]
    async fn test_success_streak_recovers_degraded() {
        use gateway::routing::config::HealthConfig;
        use gateway::routing::health::{HealthManager, HealthStatus};

        let config = HealthConfig {
            healthy_threshold: 3,
            status_codes: gateway::routing::config::StatusCodeHealthConfig {
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

        for _ in 0..3 {
            manager.update_from_result("test-auth", true, 200).await;
        }
        assert_eq!(manager.get_status("test-auth").await, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_unhealthy_blocked_from_selection() {
        use gateway::routing::config::HealthConfig;
        use gateway::routing::health::{HealthManager, HealthStatus};

        let config = HealthConfig {
            unhealthy_threshold: 2,
            cooldown_period_seconds: 10,
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        for _ in 0..2 {
            manager.update_from_result("test-auth", false, 500).await;
        }
        assert_eq!(
            manager.get_status("test-auth").await,
            HealthStatus::Unhealthy
        );
        assert!(!manager.is_available("test-auth").await);
    }

    #[tokio::test]
    async fn test_cooldown_expiration_allows_recovery() {
        use gateway::routing::config::HealthConfig;
        use gateway::routing::health::{HealthManager, HealthStatus};

        let config = HealthConfig {
            unhealthy_threshold: 2,
            cooldown_period_seconds: 1,
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        for _ in 0..2 {
            manager.update_from_result("test-auth", false, 500).await;
        }
        assert_eq!(
            manager.get_status("test-auth").await,
            HealthStatus::Unhealthy
        );
        assert!(!manager.is_available("test-auth").await);

        // Wait for cooldown to expire
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Cooldown expired, but still unhealthy status
        assert_eq!(
            manager.get_status("test-auth").await,
            HealthStatus::Unhealthy
        );
    }

    #[tokio::test]
    async fn test_all_health_state_transitions() {
        use gateway::routing::config::HealthConfig;
        use gateway::routing::health::{HealthManager, HealthStatus};

        let config = HealthConfig {
            status_codes: gateway::routing::config::StatusCodeHealthConfig {
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
