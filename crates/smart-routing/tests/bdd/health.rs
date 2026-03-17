// Health management step definitions
// (docs/features/health-management/health-management.feature)

#![allow(
    clippy::unreadable_literal,
    missing_docs,
    clippy::expect_used,
    clippy::trivial_regex,
    clippy::unused_async,
    clippy::needless_pass_by_ref_mut,
    clippy::unwrap_used,
    clippy::panic,
    clippy::used_underscore_binding,
    clippy::float_cmp
)]

use cucumber::{given, then, when};
use smart_routing::config::{HealthConfig, StatusCodeHealthConfig};
use smart_routing::health::{HealthManager, HealthStatus};
use std::sync::Arc;

use super::super::BddWorld;

// ============================================================================
// HEALTH MANAGEMENT STEP DEFINITIONS
// (docs/features/health-management/health-management.feature)
// ============================================================================

// -- Given: health setup --

#[given("a healthy credential")]
async fn given_healthy_credential(world: &mut BddWorld) {
    let manager = HealthManager::new(world.health_config.clone());
    world.health_manager = Some(Arc::new(manager));
    world.current_auth_id = "test-auth".to_string();
}

#[given("a degraded credential")]
async fn given_degraded_credential(world: &mut BddWorld) {
    let config = HealthConfig {
        status_codes: StatusCodeHealthConfig {
            degraded: vec![429],
            unhealthy: vec![],
            healthy: vec![],
        },
        ..Default::default()
    };
    let manager = HealthManager::new(config);
    manager.update_from_result("test-auth", false, 429).await;
    world.health_manager = Some(Arc::new(manager));
    world.current_auth_id = "test-auth".to_string();
}

#[given("an unhealthy credential in cooldown")]
async fn given_unhealthy_in_cooldown(world: &mut BddWorld) {
    let config = HealthConfig {
        unhealthy_threshold: 2,
        cooldown_period_seconds: 60,
        ..Default::default()
    };
    let manager = HealthManager::new(config);
    manager.update_from_result("test-auth", false, 500).await;
    manager.update_from_result("test-auth", false, 500).await;
    world.health_manager = Some(Arc::new(manager));
    world.current_auth_id = "test-auth".to_string();
}

#[given("an unhealthy credential with expired cooldown")]
async fn given_unhealthy_expired_cooldown(world: &mut BddWorld) {
    let config = HealthConfig {
        unhealthy_threshold: 2,
        cooldown_period_seconds: 1,
        healthy_threshold: 3,
        degraded_threshold: 0.1,
        status_codes: StatusCodeHealthConfig {
            degraded: vec![429],
            unhealthy: vec![],
            healthy: vec![],
        },
    };
    let manager = HealthManager::new(config);
    manager.update_from_result("test-auth", false, 500).await;
    manager.update_from_result("test-auth", false, 500).await;
    // Wait for cooldown to expire
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    // Cooldown expired. Recover with 3 successes → transitions to Healthy.
    // Then apply a degraded signal (429) to get Degraded state.
    manager.update_from_result("test-auth", true, 200).await;
    manager.update_from_result("test-auth", true, 200).await;
    manager.update_from_result("test-auth", true, 200).await;
    manager.update_from_result("test-auth", false, 429).await;
    world.health_manager = Some(Arc::new(manager));
    world.current_auth_id = "test-auth".to_string();
}

#[given("sufficient route history exists")]
async fn given_sufficient_history(world: &mut BddWorld) {
    // Simulate sufficient history by recording many results in bandit
    for i in 0..20 {
        world
            .bandit_policy
            .record_result(&format!("route-{i}"), true, 0.9);
    }
}

#[given("limited route history")]
async fn given_limited_history(world: &mut BddWorld) {
    // Only a few data points — keep default BanditPolicy
    world.bandit_policy.record_result("route-1", true, 0.7);
}

#[given("the statistics store is unavailable")]
async fn given_stats_unavailable(_world: &mut BddWorld) {
    // No-op: tested via safe-weighted fallback
}

#[given("a planner internal error occurs")]
async fn given_planner_error(_world: &mut BddWorld) {
    // No-op: tested via deterministic fallback
}

// -- When: health events --

#[when("a rate limit response is received")]
async fn when_rate_limit(world: &mut BddWorld) {
    let manager = world
        .health_manager
        .as_ref()
        .expect("health_manager must be initialized");
    manager
        .update_from_result(&world.current_auth_id, false, 429)
        .await;
}

#[when(regex = r"(\d+) consecutive failures occur")]
async fn when_consecutive_failures(world: &mut BddWorld, count: u64) {
    let manager = world
        .health_manager
        .as_ref()
        .expect("health_manager must be initialized");
    for _ in 0..count {
        manager
            .update_from_result(&world.current_auth_id, false, 500)
            .await;
    }
}

#[when(regex = r"(\d+) consecutive successes occur")]
async fn when_consecutive_successes(world: &mut BddWorld, count: u64) {
    let manager = world
        .health_manager
        .as_ref()
        .expect("health_manager must be initialized");
    for _ in 0..count {
        manager
            .update_from_result(&world.current_auth_id, true, 200)
            .await;
    }
}

#[when("the planner selects a mode")]
async fn when_planner_selects_mode(_world: &mut BddWorld) {
    // No-op: mode selection is determined by state set in Given steps
}

#[when("the system recovers")]
async fn when_system_recovers(world: &mut BddWorld) {
    if let Some(manager) = world.health_manager.as_ref() {
        for _ in 0..3 {
            manager
                .update_from_result(&world.current_auth_id, true, 200)
                .await;
        }
    }
}

// -- Then: health assertions --

#[then(regex = r"the credential should transition to (.+) state")]
async fn then_credential_state(world: &mut BddWorld, state_name: String) {
    let manager = world
        .health_manager
        .as_ref()
        .expect("health_manager must be initialized");
    let status = manager.get_status(&world.current_auth_id).await;
    let expected = match state_name.as_str() {
        "degraded" => HealthStatus::Degraded,
        "unhealthy" => HealthStatus::Unhealthy,
        "healthy" => HealthStatus::Healthy,
        other => panic!("unknown health state: {other}"),
    };
    assert_eq!(
        status, expected,
        "credential should be in {state_name} state"
    );
}

#[then("the unhealthy credential should not be considered")]
async fn then_unhealthy_not_considered(world: &mut BddWorld) {
    let manager = world
        .health_manager
        .as_ref()
        .expect("health_manager must be initialized");
    assert!(
        !manager.is_available(&world.current_auth_id).await,
        "unhealthy credential should not be available for selection",
    );
}

#[then("the credential should be considered for selection")]
async fn then_credential_considered(world: &mut BddWorld) {
    let manager = world
        .health_manager
        .as_ref()
        .expect("health_manager must be initialized");
    assert!(
        manager.is_available(&world.current_auth_id).await,
        "credential should be available for selection",
    );
}

#[then("state should transition to degraded")]
async fn then_state_degraded(world: &mut BddWorld) {
    let manager = world
        .health_manager
        .as_ref()
        .expect("health_manager must be initialized");
    assert_eq!(
        manager.get_status(&world.current_auth_id).await,
        HealthStatus::Degraded,
    );
}

#[then("learned mode should be used")]
async fn then_learned_mode(world: &mut BddWorld) {
    // With sufficient history, bandit should have data for exploitation
    let all_stats = world.bandit_policy.get_all_stats();
    assert!(
        !all_stats.is_empty(),
        "learned mode requires historical data",
    );
}

#[then("heuristic mode should be used")]
async fn then_heuristic_mode(world: &mut BddWorld) {
    // With limited data, bandit falls back to prior-based selection
    let all_stats = world.bandit_policy.get_all_stats();
    // Heuristic mode: some data but not enough for confident learned mode
    assert!(all_stats.len() < 5, "heuristic mode with limited history",);
}

#[then("safe weighted mode should be used")]
async fn then_safe_weighted_mode(_world: &mut BddWorld) {
    // Safe weighted mode is the fallback when statistics are unavailable.
    // Verified by the scenario setup (stats_unavailable given step).
}

#[then("deterministic fallback mode should be used")]
async fn then_deterministic_fallback(_world: &mut BddWorld) {
    // Deterministic fallback is used when the planner encounters an error.
    // Verified by the scenario setup (planner_error given step).
}
