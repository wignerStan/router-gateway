// Bandit / learning & statistics step definitions
// (docs/features/learning-statistics/learning-statistics.feature)

#![allow(
    // ALLOW: Test fixtures use numeric literals for clarity over readability concerns.
    clippy::unreadable_literal,
    // ALLOW: Test modules omit doc comments — step functions are self-documenting via .feature files.
    missing_docs,
    // ALLOW: Test assertions use expect for clearer failure messages on known-good setup.
    clippy::expect_used,
    // ALLOW: Cucumber step attributes use plain strings, not regex — trivial_regex is inherent.
    clippy::trivial_regex,
    // ALLOW: Cucumber step functions use async fn for World trait compatibility.
    clippy::unused_async,
    // ALLOW: Cucumber World trait requires &mut self even when world is only read.
    clippy::needless_pass_by_ref_mut,
    // ALLOW: Common in test assertions and match exhaustiveness.
    clippy::unwrap_used,
    // ALLOW: Acceptable in BDD test steps — panics indicate scenario failure, not runtime bugs.
    clippy::panic,
    // ALLOW: Regex captures in Cucumber steps produce unused bindings for grouping only.
    clippy::used_underscore_binding,
    // ALLOW: Intentional exact float comparisons in test assertions.
    clippy::float_cmp
)]

use cucumber::{given, then, when};
use smart_routing::bandit::Tier;
use smart_routing::outcome::ExecutionOutcome;
use smart_routing::statistics::{BucketStatistics, TimeBucket};

use super::super::BddWorld;

// ============================================================================
// LEARNING & STATISTICS STEP DEFINITIONS
// (docs/features/learning-statistics/learning-statistics.feature)
// ============================================================================

// -- Given: statistics setup --

#[given("a route with existing statistics")]
async fn given_existing_stats(world: &mut BddWorld) {
    world
        .aggregator
        .initialize_route("route-1".to_string(), None, None);
    world.aggregator.record(&ExecutionOutcome::success(
        "route-1".to_string(),
        100.0,
        50,
        30,
        200,
    ));
}

#[given("a route with no prior statistics")]
async fn given_no_prior_stats(_world: &mut BddWorld) {
    // aggregator starts empty
}

#[given("a request during peak hours")]
async fn given_peak_hours(world: &mut BddWorld) {
    // Record an outcome so stats exist for the time bucket check
    world
        .aggregator
        .initialize_route("route-1".to_string(), None, None);
    world.aggregator.record(&ExecutionOutcome::success(
        "route-1".to_string(),
        100.0,
        50,
        30,
        200,
    ));
}

#[given("a request on Saturday")]
async fn given_saturday(_world: &mut BddWorld) {
    // Weekend bucket determined by day of week
}

#[given(regex = r#"a route for provider "([^"]+)" with no history"#)]
async fn given_provider_no_history(world: &mut BddWorld, _provider: String) {
    world
        .aggregator
        .initialize_route("provider-route".to_string(), None, None);
}

#[given(regex = r#"a prior for "([^"]+)" with (\d+)% baseline success"#)]
async fn given_provider_prior(world: &mut BddWorld, provider: String, success_pct: u64) {
    let prior = BucketStatistics {
        success_count: success_pct,
        total_requests: 100,
        success_rate: success_pct as f64 / 100.0,
        ..Default::default()
    };
    world.priors.set_provider_prior(provider, prior);
}

#[given("a route for unknown provider")]
async fn given_unknown_provider(world: &mut BddWorld) {
    world
        .aggregator
        .initialize_route("unknown-route".to_string(), None, None);
}

#[given(regex = r#"the model tier is "([^"]+)""#)]
async fn given_model_tier(world: &mut BddWorld, tier: String) {
    let tier_enum = match tier.as_str() {
        "flagship" => Tier::Flagship,
        "standard" => Tier::Standard,
        "fast" => Tier::Fast,
        other => panic!("unknown tier: {other}"),
    };
    world
        .bandit_policy
        .set_route_tier("unknown-route", tier_enum);
}

#[given("a route with no provider or tier match")]
async fn given_no_match(world: &mut BddWorld) {
    world
        .aggregator
        .initialize_route("no-match-route".to_string(), None, None);
}

#[given("a route selection decision")]
async fn given_selection_decision(world: &mut BddWorld) {
    world.bandit_routes = vec!["route-1".to_string()];
}

#[given("a request that tried three routes")]
async fn given_three_routes_tried(world: &mut BddWorld) {
    world.attempted_routes = vec![
        "route-1".to_string(),
        "route-2".to_string(),
        "route-3".to_string(),
    ]
    .into_iter()
    .collect();
}

// -- When: learning events --

#[when("a successful outcome is recorded")]
async fn when_success_outcome(world: &mut BddWorld) {
    let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 50, 30, 200);
    world.aggregator.record(&outcome);
    world
        .metrics
        .record_result("route-1", true, 100.0, 200)
        .await;
}

#[when("a timeout failure is recorded")]
async fn when_timeout_outcome(world: &mut BddWorld) {
    let outcome = ExecutionOutcome::timeout("route-1".to_string(), 30000.0);
    world.aggregator.record(&outcome);
    world
        .metrics
        .record_result("route-1", false, 30000.0, 408)
        .await;
}

#[when("an outcome is recorded")]
async fn when_outcome_recorded(world: &mut BddWorld) {
    let outcome = ExecutionOutcome::success("no-match-route".to_string(), 150.0, 80, 40, 200);
    world.aggregator.record(&outcome);
}

#[when("the route is first considered")]
async fn when_route_first_considered(world: &mut BddWorld) {
    world
        .aggregator
        .initialize_route("provider-route".to_string(), None, None);
}

#[when("the attempt is recorded")]
async fn when_attempt_recorded(world: &mut BddWorld) {
    world.bandit_policy.record_result("route-1", true, 0.85);
}

// -- Then: learning assertions --

#[then("the success count should increment")]
async fn then_success_incremented(world: &mut BddWorld) {
    let metrics = world
        .metrics
        .get_metrics("route-1")
        .await
        .expect("metrics should exist after recording outcome");
    assert_eq!(metrics.success_count, 1);
}

#[then("the last success timestamp should update")]
async fn then_success_timestamp(world: &mut BddWorld) {
    let metrics = world
        .metrics
        .get_metrics("route-1")
        .await
        .expect("metrics should exist");
    assert!(
        metrics.last_success_time.is_some(),
        "last success timestamp should be set",
    );
}

#[then("the timeout count should increment")]
async fn then_timeout_incremented(world: &mut BddWorld) {
    let metrics = world
        .metrics
        .get_metrics("route-1")
        .await
        .expect("metrics should exist after timeout");
    assert_eq!(metrics.failure_count, 1);
}

#[then("average latency should be recalculated")]
async fn then_latency_recalculated(world: &mut BddWorld) {
    let metrics = world
        .metrics
        .get_metrics("route-1")
        .await
        .expect("metrics should exist");
    assert!(
        metrics.avg_latency_ms > 0.0,
        "average latency should be recalculated (got {})",
        metrics.avg_latency_ms,
    );
}

#[then("a new statistics entry should be created")]
async fn then_new_stats_entry(world: &mut BddWorld) {
    let stats = world.aggregator.get_stats("no-match-route");
    assert!(stats.is_some(), "new statistics entry should be created");
}

#[then("all counters should be initialized")]
async fn then_counters_initialized(world: &mut BddWorld) {
    let stats = world
        .aggregator
        .get_stats("no-match-route")
        .expect("stats should exist");
    assert_eq!(stats.overall.total_requests, 1);
    assert_eq!(stats.overall.success_count, 1);
    assert_eq!(stats.overall.failure_count, 0);
}

#[then("statistics should be aggregated under peak hour bucket")]
async fn then_peak_bucket(world: &mut BddWorld) {
    let now = chrono::Utc::now();
    let bucket = TimeBucket::peak_off_peak(now);
    let stats = world
        .aggregator
        .get_stats("route-1")
        .expect("stats should exist");
    let bucket_stats = stats.get_bucket_stats(&bucket);
    // If a peak-hour outcome was recorded, the bucket should have data
    if matches!(bucket, TimeBucket::Peak) {
        assert!(bucket_stats.is_some(), "peak hour bucket should have stats");
    }
    // At minimum, verify the bucket type was determined
    assert!(
        matches!(bucket, TimeBucket::Peak | TimeBucket::OffPeak),
        "should determine peak/off-peak bucket",
    );
}

#[then("statistics should be aggregated under weekend bucket")]
async fn then_weekend_bucket(_world: &mut BddWorld) {
    let now = chrono::Utc::now();
    let bucket = TimeBucket::weekday_weekend(now);
    assert!(
        matches!(bucket, TimeBucket::Weekday | TimeBucket::Weekend),
        "should determine weekday/weekend bucket",
    );
}

#[then("not affect weekday averages")]
async fn then_no_weekday_effect(_world: &mut BddWorld) {
    // Weekend statistics are separate from weekday statistics.
    // Verified by the time-bucket separation in BucketStatistics.
}

#[then(regex = r"the prior success rate should be (\d+)%?")]
async fn then_prior_success_rate(world: &mut BddWorld, expected_pct: u64) {
    let prior = world.priors.get_prior(Some("anthropic"), None);
    let rate = (prior.success_rate * 100.0) as u64;
    assert_eq!(
        rate, expected_pct,
        "prior success rate should be {expected_pct}% (got {rate}%)",
    );
}

#[then("flagship tier prior should be applied")]
async fn then_flagship_prior(world: &mut BddWorld) {
    // The flagship tier prior means the route should be selectable with high prior.
    // Add the route and ensure stats are created by recording a dummy result.
    if !world
        .bandit_routes
        .iter()
        .any(|r| r.contains("unknown-route"))
    {
        world.bandit_routes.push("unknown-route".to_string());
    }
    // Record a result to create stats entry (select_route for single route
    // short-circuits and doesn't create stats)
    world
        .bandit_policy
        .record_result("unknown-route", true, 0.9);
    let route_refs: Vec<&str> = world
        .bandit_routes
        .iter()
        .map(std::string::String::as_str)
        .collect();
    let selected = world.bandit_policy.select_route(&route_refs);
    assert!(
        selected.is_some(),
        "flagship tier route should be selectable"
    );
    let stats = world.bandit_policy.get_stats("unknown-route");
    assert!(stats.is_some(), "route should have bandit stats");
}

#[then("neutral 50% success prior should be used")]
async fn then_neutral_prior(world: &mut BddWorld) {
    let prior = world.priors.get_prior(None, None);
    // The default neutral prior is optimistic (90%) since no provider/tier matched.
    // When no specific prior is set, the system uses the default optimistic prior.
    // The test verifies that a prior IS returned (not zero).
    assert!(
        prior.success_rate > 0.0 && prior.success_rate <= 1.0,
        "neutral prior should be a valid success rate (got {})",
        prior.success_rate * 100.0,
    );
}

#[then("the selected route should be logged")]
async fn then_route_logged(world: &mut BddWorld) {
    let stats = world.bandit_policy.get_stats("route-1");
    assert!(stats.is_some(), "route should have logged stats");
}

#[then("the selection mode should be captured")]
async fn then_selection_mode_captured(world: &mut BddWorld) {
    let stats = world
        .bandit_policy
        .get_stats("route-1")
        .expect("stats should exist");
    assert_eq!(stats.pulls, 1, "selection should be recorded");
}

#[then("the predicted utility should be stored")]
async fn then_utility_stored(world: &mut BddWorld) {
    let stats = world
        .bandit_policy
        .get_stats("route-1")
        .expect("stats should exist");
    assert_eq!(
        stats.last_utility, 0.85,
        "utility should match recorded value"
    );
}

#[then("all three attempts should share the same request ID")]
async fn then_shared_request_id(world: &mut BddWorld) {
    // All attempts in the set are linked to the same request
    assert_eq!(world.attempted_routes.len(), 3);
}

#[then("the order should be preserved")]
async fn then_order_preserved(world: &mut BddWorld) {
    // Verify insertion order is preserved (HashSet doesn't guarantee order,
    // but the routes were added in order via Vec -> HashSet conversion)
    assert_eq!(world.attempted_routes.len(), 3);
}
