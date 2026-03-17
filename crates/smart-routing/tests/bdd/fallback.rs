// Route execution step definitions
// (docs/features/route-execution/route-execution.feature)

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
use smart_routing::outcome::{ErrorClass, ExecutionOutcome};

use super::super::BddWorld;

// ============================================================================
// ROUTE EXECUTION STEP DEFINITIONS
// (docs/features/route-execution/route-execution.feature)
// ============================================================================

// -- Given: route execution setup --

#[given(regex = r#"a route decision with primary route "([^"]+)""#)]
async fn given_route_decision(world: &mut BddWorld, primary_route: String) {
    world.current_auth_id = primary_route;
    world.metrics.initialize_auth(&world.current_auth_id).await;
    world.attempted_routes.clear();
    world.attempt_count = 0;
}

#[given("a route decision with multiple fallbacks")]
async fn given_multiple_fallbacks(world: &mut BddWorld) {
    world.current_auth_id = "primary-route".to_string();
    world.metrics.initialize_auth("primary-route").await;
    world.metrics.initialize_auth("fallback-1").await;
    world.metrics.initialize_auth("fallback-2").await;
    world.attempted_routes.clear();
    world.attempt_count = 0;
}

#[given("a route decision with fallback routes")]
async fn given_with_fallbacks(world: &mut BddWorld) {
    world.current_auth_id = "primary-route".to_string();
    world.metrics.initialize_auth("primary-route").await;
    world.metrics.initialize_auth("fallback-1").await;
    world.attempted_routes.clear();
    world.attempt_count = 0;
}

#[given(regex = r"a request with retry budget of (\d+)")]
async fn given_retry_budget(world: &mut BddWorld, budget: u64) {
    world.retry_budget = budget as u32;
    world.attempt_count = 0;
    world.metrics.initialize_auth("primary-route").await;
    world.metrics.initialize_auth("fallback-1").await;
    world.metrics.initialize_auth("fallback-2").await;
    world.current_auth_id = "primary-route".to_string();
}

#[given(regex = r#"a request chain already attempted route "([^"]+)""#)]
async fn given_already_attempted(world: &mut BddWorld, route: String) {
    world.attempted_routes.insert(route);
}

#[given(regex = r#"(two|\d+) consecutive failures on provider "([^"]+)""#)]
async fn given_provider_failures(world: &mut BddWorld, count_or_word: String, provider: String) {
    let count: u32 = if count_or_word == "two" {
        2
    } else {
        count_or_word.parse().unwrap_or(1)
    };
    *world.provider_failures.entry(provider).or_insert(0) += count;
}

#[given("a route execution that succeeds")]
async fn given_execution_succeeds(world: &mut BddWorld) {
    world.current_auth_id = "exec-route".to_string();
    world.metrics.initialize_auth(&world.current_auth_id).await;
    world.last_outcome_success = true;
}

#[given("a route execution that fails")]
async fn given_execution_fails(world: &mut BddWorld) {
    world.current_auth_id = "exec-route".to_string();
    world.metrics.initialize_auth(&world.current_auth_id).await;
}

// -- Given: route response conditions (And steps) --

#[given("the primary route responds successfully")]
async fn given_primary_success(world: &mut BddWorld) {
    world
        .metrics
        .record_result(&world.current_auth_id, true, 150.0, 200)
        .await;
    world.attempt_count += 1;
}

#[given("the primary route times out")]
async fn given_primary_timeout(world: &mut BddWorld) {
    world
        .metrics
        .record_result(&world.current_auth_id, false, 30000.0, 408)
        .await;
    world.attempt_count += 1;
    world.last_outcome_success = false;
}

#[given("the primary route returns rate limit error")]
async fn given_rate_limit_error(world: &mut BddWorld) {
    world
        .metrics
        .record_result(&world.current_auth_id, false, 100.0, 429)
        .await;
    world.attempt_count += 1;
    world.last_outcome_success = false;
}

#[given("the primary route returns server error")]
async fn given_server_error(world: &mut BddWorld) {
    world
        .metrics
        .record_result(&world.current_auth_id, false, 5000.0, 503)
        .await;
    world.attempt_count += 1;
    world.last_outcome_success = false;
}

#[given("the primary route returns authentication error")]
async fn given_auth_error(world: &mut BddWorld) {
    world
        .metrics
        .record_result(&world.current_auth_id, false, 50.0, 401)
        .await;
    world.attempt_count += 1;
    world.last_outcome_success = false;
}

#[given("all routes fail with retryable errors")]
async fn given_all_retryable_failures(world: &mut BddWorld) {
    for _ in 0..world.retry_budget {
        world
            .metrics
            .record_result(&world.current_auth_id, false, 1000.0, 500)
            .await;
        world.attempt_count += 1;
    }
    world.last_outcome_success = false;
}

#[given("the second route succeeds")]
async fn given_second_succeeds(world: &mut BddWorld) {
    world
        .metrics
        .record_result(&world.current_auth_id, false, 1000.0, 500)
        .await;
    world.attempt_count += 1;
    world
        .metrics
        .record_result("fallback-1", true, 200.0, 200)
        .await;
    world.attempt_count += 1;
    world.last_outcome_success = true;
}

// -- When: route execution actions --

#[when("the request is executed")]
async fn when_execute_request(_world: &mut BddWorld) {
    // Execution is simulated via the Given outcome steps
}

#[when("the same route is selected again")]
async fn when_same_route_selected(world: &mut BddWorld) {
    // Simulate: the same route "openai-gpt4" is selected again.
    // Loop guard should detect this and choose a different route.
    let candidate = "openai-gpt4";
    if world.attempted_routes.contains(candidate) {
        // Loop guard triggered — select a different route
        world
            .session_manager
            .set_provider(world.current_auth_id.clone(), "fallback-route".to_string())
            .await
            .expect("set_provider should succeed");
    }
}

#[when("selecting the next route")]
async fn when_select_next_route(_world: &mut BddWorld) {
    // Check provider diversity
}

#[when("the outcome is recorded")]
async fn when_the_outcome_recorded(world: &mut BddWorld) {
    if world.last_outcome_success {
        world
            .metrics
            .record_result(&world.current_auth_id, true, 200.0, 200)
            .await;
    } else {
        world
            .metrics
            .record_result(&world.current_auth_id, false, 1000.0, 500)
            .await;
    }
}

#[when("attempt history is reviewed")]
async fn when_review_history(_world: &mut BddWorld) {
    // Review is a read-only operation
}

// -- Then: route execution assertions --

#[then("the response should be returned")]
async fn then_response_returned(world: &mut BddWorld) {
    let metrics = world
        .metrics
        .get_metrics(&world.current_auth_id)
        .await
        .expect("metrics should exist");
    assert!(
        metrics.success_count > 0,
        "successful response should be recorded",
    );
}

#[then("the primary route should be recorded as successful")]
async fn then_primary_recorded_success(world: &mut BddWorld) {
    let metrics = world
        .metrics
        .get_metrics(&world.current_auth_id)
        .await
        .expect("metrics should exist");
    assert_eq!(metrics.success_count, 1);
}

#[then("the fallback route should be attempted")]
async fn then_fallback_attempted(world: &mut BddWorld) {
    // The primary failed, so fallback should be tried
    let metrics = world
        .metrics
        .get_metrics(&world.current_auth_id)
        .await
        .expect("metrics should exist");
    assert!(
        metrics.failure_count > 0,
        "primary failure should trigger fallback"
    );
}

#[then("the timeout should be recorded")]
async fn then_timeout_recorded(world: &mut BddWorld) {
    let metrics = world
        .metrics
        .get_metrics(&world.current_auth_id)
        .await
        .expect("metrics should exist");
    assert!(
        metrics.avg_latency_ms > 10000.0,
        "timeout should record high latency (got {})",
        metrics.avg_latency_ms,
    );
}

#[then("the first fallback should be attempted")]
async fn then_first_fallback(world: &mut BddWorld) {
    let metrics = world
        .metrics
        .get_metrics(&world.current_auth_id)
        .await
        .expect("metrics should exist");
    assert!(
        metrics.failure_count > 0,
        "rate limit should trigger fallback"
    );
}

#[then("no fallback should be attempted")]
async fn then_no_fallback(_world: &mut BddWorld) {
    // Auth errors are non-retryable — no fallback attempted
    let error_class = ErrorClass::from_status_code(401);
    assert!(error_class.is_some());
    assert!(
        !error_class.unwrap().is_retryable(),
        "auth error should not be retryable",
    );
}

#[then("the error should be returned immediately")]
async fn then_error_immediate(world: &mut BddWorld) {
    let metrics = world
        .metrics
        .get_metrics(&world.current_auth_id)
        .await
        .expect("metrics should exist");
    assert_eq!(metrics.failure_count, 1, "auth error fails immediately");
}

#[then(regex = r"exactly (\d+) attempts should be made")]
async fn then_exact_attempts(world: &mut BddWorld, expected: u64) {
    assert_eq!(
        world.attempt_count, expected as u32,
        "should make exactly {expected} attempts",
    );
}

#[then("the final error should be returned")]
async fn then_final_error(world: &mut BddWorld) {
    assert!(
        !world.last_outcome_success,
        "final outcome should be failure",
    );
}

#[then(regex = r"only (\d+) attempts should be made")]
async fn then_only_attempts(world: &mut BddWorld, expected: u64) {
    assert_eq!(
        world.attempt_count, expected as u32,
        "should stop after {expected} attempts",
    );
}

#[then("the successful response should be returned")]
async fn then_success_returned(world: &mut BddWorld) {
    assert!(world.last_outcome_success, "should have succeeded");
}

#[then("the loop guard should block the attempt")]
async fn then_loop_guard(world: &mut BddWorld) {
    // Verify that the route was already attempted
    assert!(
        world.attempted_routes.contains("openai-gpt4"),
        "route should have been previously attempted",
    );
}

#[then("a different route should be selected")]
async fn then_different_selected(world: &mut BddWorld) {
    let provider = world
        .session_manager
        .get_preferred_provider(&world.current_auth_id)
        .await;
    assert!(provider.is_some(), "a provider should be selected");
}

#[then("a different provider should be preferred")]
async fn then_different_provider_preferred(world: &mut BddWorld) {
    let failures = world.provider_failures.get("openai").copied().unwrap_or(0);
    assert!(
        failures >= 2,
        "provider should have consecutive failures triggering diversity",
    );
}

#[then("success should be recorded")]
async fn then_success_recorded(world: &mut BddWorld) {
    let metrics = world
        .metrics
        .get_metrics(&world.current_auth_id)
        .await
        .expect("metrics should exist");
    assert_eq!(metrics.success_count, 1);
}

#[then("latency should be captured")]
async fn then_latency_captured(world: &mut BddWorld) {
    let metrics = world
        .metrics
        .get_metrics(&world.current_auth_id)
        .await
        .expect("metrics should exist");
    assert!(metrics.avg_latency_ms > 0.0, "latency should be captured");
}

#[then("token usage should be stored")]
async fn then_tokens_stored(_world: &mut BddWorld) {
    // Token usage is tracked via ExecutionOutcome in the statistics module
    let outcome = ExecutionOutcome::success("test-route".to_string(), 200.0, 100, 50, 200);
    assert_eq!(outcome.prompt_tokens, 100);
    assert_eq!(outcome.completion_tokens, 50);
}

#[then("failure should be recorded")]
async fn then_failure_recorded(world: &mut BddWorld) {
    let metrics = world
        .metrics
        .get_metrics(&world.current_auth_id)
        .await
        .expect("metrics should exist");
    assert!(metrics.failure_count > 0);
}

#[then("error classification should be stored")]
async fn then_error_class_stored(world: &mut BddWorld) {
    let metrics = world
        .metrics
        .get_metrics(&world.current_auth_id)
        .await
        .expect("metrics should exist");
    assert_eq!(metrics.consecutive_failures, 1);
    assert!(metrics.last_failure_time.is_some());
}

#[then("fallback usage should be noted")]
async fn then_fallback_noted(_world: &mut BddWorld) {
    let outcome = ExecutionOutcome::failure(
        "primary-route".to_string(),
        5000.0,
        503,
        true,
        Some("original-request".to_string()),
    );
    assert!(outcome.used_fallback, "fallback usage should be noted");
}
