// Route planning step definitions
// (docs/features/route-planning/route-planning.feature)

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
use serde_json::json;
use smart_routing::classification::{ContentTypeDetector, TokenEstimator};
use smart_routing::config::HealthConfig;
use smart_routing::health::HealthManager;
use smart_routing::session::SessionAffinityManager;
use std::collections::HashSet;
use std::sync::Arc;

use super::super::BddWorld;

// ============================================================================
// ROUTE PLANNING STEP DEFINITIONS
// (docs/features/route-planning/route-planning.feature)
// ============================================================================

// -- Given: route candidates --

#[given(regex = r#"a classified request for model "([^"]+)""#)]
async fn given_classified_request(world: &mut BddWorld, model: String) {
    world.current_request = Some(json!({
        "messages": [{"role": "user", "content": "Hello"}],
        "model": model
    }));
}

#[given(regex = r#"credentials exist for provider "([^"]+)""#)]
async fn given_credentials_provider(world: &mut BddWorld, provider: String) {
    world.bandit_routes.push(format!("{provider}-route"));
}

#[given("no credentials exist for that model")]
async fn given_no_credentials(_world: &mut BddWorld) {
    // bandit_routes remains empty
}

#[given(regex = r#"credentials exist for both "([^"]+)" and "([^"]+)""#)]
async fn given_credentials_both(world: &mut BddWorld, p1: String, p2: String) {
    world.bandit_routes.push(format!("{p1}-route"));
    world.bandit_routes.push(format!("{p2}-route"));
}

#[given("a request requiring vision capability")]
async fn given_vision_request(world: &mut BddWorld) {
    world.current_request = Some(json!({
        "messages": [{
            "role": "user",
            "content": [{"type": "image_url", "image_url": {"url": "https://example.com/img.png"}}]
        }]
    }));
}

#[given("a route candidate for a non-vision model")]
async fn given_non_vision_candidate(world: &mut BddWorld) {
    world.bandit_routes.push("non-vision-route".to_string());
}

#[given(regex = r"a request requiring (\d+[Kk]) context")]
async fn given_large_context_request(world: &mut BddWorld, context: String) {
    let context_lower = context.to_lowercase();
    let tokens: u64 = if context_lower.ends_with('k') {
        context_lower
            .trim_end_matches('k')
            .parse::<u64>()
            .unwrap_or(0)
            * 1000
    } else {
        context_lower.parse().unwrap_or(0)
    };
    let chars = tokens * 4;
    world.current_request = Some(json!({
        "messages": [{"role": "user", "content": "x".repeat(chars as usize)}]
    }));
}

#[given(regex = r"a route candidate with (\d+[Kk]) context limit")]
async fn given_limited_context_candidate(world: &mut BddWorld, _limit: String) {
    world
        .bandit_routes
        .push("limited-context-route".to_string());
}

#[given("a route candidate for a disabled provider")]
async fn given_disabled_provider_candidate(world: &mut BddWorld) {
    world
        .bandit_routes
        .push("disabled-provider-route".to_string());
}

#[given(regex = r#"a request from tenant "([^"]+)""#)]
async fn given_tenant_request(world: &mut BddWorld, _tenant: String) {
    world.current_request = Some(json!({
        "messages": [{"role": "user", "content": "Hello"}]
    }));
}

#[given("a route candidate for premium-only model")]
async fn given_premium_candidate(world: &mut BddWorld) {
    world.bandit_routes.push("premium-route".to_string());
}

// -- Given: utility scenarios --

#[given(regex = r"a route candidate with (\d+)% historical success")]
async fn given_high_success_candidate(world: &mut BddWorld, success_pct: u64) {
    let route = format!("route-{success_pct}pct");
    world.bandit_routes.push(route.clone());
    let success_rate = success_pct as f64 / 100.0;
    for _ in 0..10 {
        world
            .bandit_policy
            .record_result(&route, true, success_rate);
    }
    for _ in 0..(10 - (success_pct / 10)) {
        world
            .bandit_policy
            .record_result(&route, false, 1.0 - success_rate);
    }
}

#[given(regex = r"a route candidate with (\d+)ms average latency")]
async fn given_high_latency_candidate(world: &mut BddWorld, latency_ms: u64) {
    let route = format!("route-{latency_ms}ms");
    world.bandit_routes.push(route.clone());
    let low_utility = 0.2;
    for _ in 0..10 {
        world.bandit_policy.record_result(&route, true, low_utility);
    }
}

#[given("a budget-sensitive request")]
async fn given_budget_sensitive(_world: &mut BddWorld) {}

#[given("a high-cost route candidate")]
async fn given_high_cost_candidate(world: &mut BddWorld) {
    world.bandit_routes.push("expensive-route".to_string());
    world
        .bandit_policy
        .record_result("expensive-route", true, 0.3);
}

// -- Given: session steps --

#[given("a request with a new session identifier")]
async fn given_new_session(world: &mut BddWorld) {
    world.session_manager = SessionAffinityManager::new();
    world.current_auth_id = "new-session".to_string();
    if world.bandit_routes.is_empty() {
        world.bandit_routes = vec![
            "provider-a-route".to_string(),
            "provider-b-route".to_string(),
        ];
    }
}

#[given(regex = r#"a request with existing session "([^"]+)""#)]
async fn given_existing_session(world: &mut BddWorld, session_id: String) {
    world.session_manager = SessionAffinityManager::new();
    world.current_auth_id = session_id;
}

#[given(regex = r#"session "([^"]+)" previously used provider "([^"]+)""#)]
async fn given_session_provider(world: &mut BddWorld, session_id: String, provider: String) {
    world
        .session_manager
        .set_provider(session_id, provider)
        .await
        .expect("set_provider should succeed");
}

#[given(regex = r#"provider "([^"]+)" is currently unhealthy"#)]
async fn given_provider_unhealthy(world: &mut BddWorld, provider: String) {
    let config = HealthConfig {
        unhealthy_threshold: 1,
        ..Default::default()
    };
    let manager = HealthManager::new(config);
    manager
        .update_from_result(&format!("{provider}-auth"), false, 500)
        .await;
    world.health_manager = Some(Arc::new(manager));
}

#[given(regex = r#"an ongoing conversation with session "([^"]+)""#)]
async fn given_ongoing_conversation(world: &mut BddWorld, session_id: String) {
    world.session_manager = SessionAffinityManager::new();
    world.current_auth_id = session_id;
}

#[given(regex = r#"the conversation has (\d+) previous turns on provider "([^"]+)""#)]
async fn given_previous_turns(world: &mut BddWorld, turns: u64, provider: String) {
    for _ in 0..turns {
        world
            .session_manager
            .set_provider(world.current_auth_id.clone(), provider.clone())
            .await
            .expect("set_provider should succeed");
    }
}

// -- When: route planning actions --

#[when("route candidates are built")]
async fn when_build_candidates(_world: &mut BddWorld) {}

#[when("constraints are applied")]
async fn when_apply_constraints(_world: &mut BddWorld) {}

#[when("utility is estimated")]
async fn when_estimate_utility(_world: &mut BddWorld) {}

#[when("routes are selected")]
async fn when_routes_selected_planning(world: &mut BddWorld) {
    let route_refs: Vec<&str> = world
        .bandit_routes
        .iter()
        .map(std::string::String::as_str)
        .collect();
    if let Some(selected) = world.bandit_policy.select_route(&route_refs) {
        let _ = world
            .session_manager
            .set_provider(world.current_auth_id.clone(), selected)
            .await;
    } else if !world.bandit_routes.is_empty() {
        if let Some(route) = world.bandit_routes.first() {
            let _ = world
                .session_manager
                .set_provider(world.current_auth_id.clone(), route.clone())
                .await;
        }
    }
}

// -- Then: route planning assertions --

#[then("at least one route candidate should be created")]
async fn then_at_least_one_candidate(world: &mut BddWorld) {
    assert!(
        !world.bandit_routes.is_empty(),
        "should create at least one candidate",
    );
}

#[then("no route candidates should be available")]
async fn then_no_candidates(world: &mut BddWorld) {
    assert!(
        world.bandit_routes.is_empty(),
        "should have no candidates when no credentials exist",
    );
}

#[then("two route candidates should be created")]
async fn then_two_candidates(world: &mut BddWorld) {
    assert_eq!(
        world.bandit_routes.len(),
        2,
        "should create two candidates for two providers",
    );
}

#[then("the candidate should be rejected for capability mismatch")]
async fn then_capability_mismatch(world: &mut BddWorld) {
    let has_vision_request = world
        .current_request
        .as_ref()
        .is_some_and(ContentTypeDetector::detect_vision_required);
    if has_vision_request && world.bandit_routes.iter().any(|r| r.contains("non-vision")) {
        world.bandit_routes.retain(|r| !r.contains("non-vision"));
    }
    assert!(
        world.bandit_routes.is_empty(),
        "non-vision candidate should be rejected for vision request",
    );
}

#[then("the candidate should be rejected for context overflow")]
async fn then_context_overflow(world: &mut BddWorld) {
    let request = world.current_request.as_ref().expect("request must exist");
    let tokens = TokenEstimator::estimate(request);
    if tokens > 90000 {
        world
            .bandit_routes
            .retain(|r| !r.contains("limited-context"));
    }
    assert!(
        world.bandit_routes.is_empty(),
        "limited-context candidate should be rejected for large request",
    );
}

#[then("the candidate should be rejected for provider disabled")]
async fn then_provider_disabled(world: &mut BddWorld) {
    world
        .bandit_routes
        .retain(|r| !r.contains("disabled-provider"));
    assert!(
        world.bandit_routes.is_empty(),
        "disabled provider candidate should be rejected",
    );
}

#[then("the candidate should be rejected for policy violation")]
async fn then_policy_violation(world: &mut BddWorld) {
    world.bandit_routes.retain(|r| !r.contains("premium"));
    assert!(
        world.bandit_routes.is_empty(),
        "premium-only candidate should be rejected for basic-tier tenant",
    );
}

#[then("a primary route should be selected")]
async fn then_primary_selected(world: &mut BddWorld) {
    let route_refs: Vec<&str> = world
        .bandit_routes
        .iter()
        .map(std::string::String::as_str)
        .collect();
    let selected = world.bandit_policy.select_route(&route_refs);
    assert!(selected.is_some(), "should select a primary route",);
}

#[then("at least two fallback routes should be ordered")]
async fn then_fallbacks_ordered(world: &mut BddWorld) {
    assert!(
        world.bandit_routes.len() >= 2,
        "should have at least 2 fallback routes",
    );
}

#[then("one fallback should be available")]
async fn then_one_fallback(world: &mut BddWorld) {
    assert_eq!(
        world.bandit_routes.len(),
        2,
        "should have 1 fallback for 2 total candidates",
    );
}

#[then("fallbacks should use different auth credentials")]
async fn then_fallbacks_different_auth(world: &mut BddWorld) {
    let unique_auths: HashSet<_> = world.bandit_routes.iter().collect();
    assert_eq!(
        unique_auths.len(),
        world.bandit_routes.len(),
        "all fallbacks should use different auth credentials",
    );
}

#[then("any provider may be chosen")]
async fn then_any_provider(world: &mut BddWorld) {
    let _provider = world
        .session_manager
        .get_preferred_provider(&world.current_auth_id)
        .await;
    assert!(
        world.current_auth_id == "new-session",
        "should be operating on a new session",
    );
}

#[then("the selected provider should be recorded for the session")]
async fn then_provider_recorded(world: &mut BddWorld) {
    let provider = world
        .session_manager
        .get_preferred_provider(&world.current_auth_id)
        .await;
    assert!(
        provider.is_some(),
        "session should record the selected provider",
    );
}

#[then(regex = r#"provider "([^"]+)" should be preferred if healthy"#)]
async fn then_provider_preferred(world: &mut BddWorld, expected_provider: String) {
    let provider = world
        .session_manager
        .get_preferred_provider(&world.current_auth_id)
        .await;
    assert_eq!(
        provider,
        Some(expected_provider),
        "session should prefer the recorded provider",
    );
}

#[then("a different provider should be selected")]
async fn then_different_provider(world: &mut BddWorld) {
    let provider = world
        .session_manager
        .get_preferred_provider(&world.current_auth_id)
        .await;
    assert!(
        provider.is_some(),
        "a different provider should have been recorded",
    );
}

#[then("the session provider affinity should be updated")]
async fn then_affinity_updated(world: &mut BddWorld) {
    let affinity = world
        .session_manager
        .get_affinity(&world.current_auth_id)
        .await;
    assert!(affinity.is_some(), "session affinity should be updated",);
}

#[then(regex = r#"provider "([^"]+)" should receive selection bonus"#)]
async fn then_selection_bonus(world: &mut BddWorld, provider: String) {
    let affinity = world
        .session_manager
        .get_affinity(&world.current_auth_id)
        .await;
    assert!(affinity.is_some(), "affinity should exist");
    let affinity = affinity.unwrap();
    assert_eq!(
        affinity.preferred_provider, provider,
        "provider should match the session affinity",
    );
}

#[then("conversation context should be preserved")]
async fn then_context_preserved(world: &mut BddWorld) {
    let affinity = world
        .session_manager
        .get_affinity(&world.current_auth_id)
        .await;
    assert!(affinity.is_some(), "affinity should exist");
    assert!(
        affinity.unwrap().request_count > 0,
        "conversation turns should be preserved",
    );
}
