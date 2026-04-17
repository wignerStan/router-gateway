use super::matcher::PolicyMatcher;
use super::registry::PolicyRegistry;
use super::templates;
use super::types::*;
use crate::registry::categories::{
    CapabilityCategory, ContextWindowCategory, CostCategory, ProviderCategory, TierCategory,
};
use crate::registry::info::{DataSource, ModelCapabilities, ModelInfo, RateLimits};
use pretty_assertions::assert_eq;

fn create_test_model(id: &str, provider: &str, price: f64, context: usize) -> ModelInfo {
    ModelInfo {
        id: id.to_string(),
        name: "Test Model".to_string(),
        provider: provider.to_string(),
        context_window: context,
        max_output_tokens: 4096,
        input_price_per_million: price,
        output_price_per_million: price * 2.0,
        capabilities: ModelCapabilities {
            streaming: true,
            tools: true,
            vision: true,
            thinking: false,
        },
        rate_limits: RateLimits {
            requests_per_minute: 60,
            tokens_per_minute: 90_000,
        },
        source: DataSource::Static,
    }
}

#[test]
fn test_matcher_basic_matching() {
    let mut registry = PolicyRegistry::new();
    registry.add(templates::vision_required());

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let policies = matcher.evaluate(&model, &context);
    assert!(
        !policies.is_empty(),
        "Vision policy should match vision-capable model"
    );
}

#[test]
fn test_matcher_tier_filtering() {
    let mut registry = PolicyRegistry::new();
    registry.add(templates::performance_first()); // Fast tier only

    let matcher = PolicyMatcher::new(registry);

    // Fast model (price <= 1.0)
    let fast_model = create_test_model("fast-model", "test", 0.5, 100_000);
    let context = PolicyContext::default();

    let policies = matcher.evaluate(&fast_model, &context);
    assert!(
        !policies.is_empty(),
        "Performance policy should match fast model"
    );

    // Flagship model (high price)
    let flagship_model = create_test_model("flagship-model", "test", 20.0, 200_000);
    let policies = matcher.evaluate(&flagship_model, &context);
    assert!(
        policies.is_empty(),
        "Performance policy should not match flagship model"
    );
}

#[test]
fn test_matcher_provider_filtering() {
    let mut registry = PolicyRegistry::new();
    registry.add(templates::prefer_provider(ProviderCategory::OpenAI));

    let matcher = PolicyMatcher::new(registry);

    let openai_model = create_test_model("test", "openai", 30.0, 128_000);
    let context = PolicyContext::default();

    let policies = matcher.evaluate(&openai_model, &context);
    assert!(!policies.is_empty(), "Should match OpenAI provider");
}

#[test]
fn test_matcher_weight_factor() {
    let mut registry = PolicyRegistry::new();
    registry.add(
        RoutingPolicy::new("boost_test", "Boost Test")
            .with_priority(10)
            .with_action("weight")
            .with_weight_factor(2.0),
    );

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 1.0, 100_000);
    let context = PolicyContext::default();

    let factor = matcher.calculate_weight_factor(&model, &context);
    assert!(factor > 1.0, "Weight factor should be boosted");
}

#[test]
fn test_matcher_blocking() {
    let mut registry = PolicyRegistry::new();

    // Create a block policy that filters by cost (ultra_premium)
    let mut block_policy = RoutingPolicy::new("block_expensive", "Block Ultra Premium")
        .with_priority(100)
        .with_action("block");
    block_policy.filters.costs.push(CostCategory::UltraPremium);
    registry.add(block_policy);

    let matcher = PolicyMatcher::new(registry);

    // Ultra premium model should be blocked
    let expensive_model = create_test_model("expensive", "test", 60.0, 100_000);
    let context = PolicyContext::default();
    assert!(
        matcher.is_blocked(&expensive_model, &context),
        "Ultra premium model should be blocked"
    );

    // Standard cost model should not be blocked
    let cheap_model = create_test_model("cheap", "test", 3.0, 100_000);
    assert!(
        !matcher.is_blocked(&cheap_model, &context),
        "Standard cost model should not be blocked"
    );
}

#[test]
fn test_matcher_best_match() {
    let mut registry = PolicyRegistry::new();
    registry.add(templates::vision_required().with_priority(10));
    registry.add(templates::quality_first().with_priority(30));

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 15.0, 200_000);
    let context = PolicyContext::default();

    let best = matcher.evaluate_best(&model, &context);
    assert!(best.is_some());

    // Quality first has higher priority, should be selected
    let best = best.unwrap();
    assert_eq!(best.policy.id, "quality_first");
}

#[test]
fn test_matcher_multi_dimension() {
    // Test combining multiple dimensions
    let mut registry = PolicyRegistry::new();
    let multi_policy = RoutingPolicy::new("multi", "Multi-dimensional Policy")
        .with_priority(50)
        .with_capability(CapabilityCategory::Vision, "require")
        .with_tier(TierCategory::Standard)
        .with_provider(ProviderCategory::OpenAI)
        .with_action("prefer");

    registry.add(multi_policy);

    let matcher = PolicyMatcher::new(registry);

    // Model matching all dimensions
    let matching_model = create_test_model("test", "openai", 3.0, 200_000);
    let context = PolicyContext::default();
    let policies = matcher.evaluate(&matching_model, &context);
    assert!(!policies.is_empty(), "Should match all dimensions");

    // Model missing vision
    let mut non_vision_model = create_test_model("text-only", "openai", 3.0, 200_000);
    non_vision_model.capabilities.vision = false;
    let policies = matcher.evaluate(&non_vision_model, &context);
    assert!(policies.is_empty(), "Should not match - no vision");
}

// ========================================
// Condition Operator Tests
// ========================================

#[test]
fn test_condition_operator_eq() {
    let mut policy = RoutingPolicy::new("test", "Test Policy");
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TimeOfDay,
        value: "10".to_string(),
        operator: "eq".to_string(),
    });

    let context_eq = PolicyContext {
        hour_of_day: Some(10),
        ..Default::default()
    };
    assert!(
        policy.matches(&context_eq),
        "eq operator should match equal value"
    );

    let context_ne = PolicyContext {
        hour_of_day: Some(11),
        ..Default::default()
    };
    assert!(
        !policy.matches(&context_ne),
        "eq operator should not match different value"
    );
}

#[test]
fn test_condition_operator_ne() {
    let mut policy = RoutingPolicy::new("test", "Test Policy");
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TimeOfDay,
        value: "10".to_string(),
        operator: "ne".to_string(),
    });

    let context_ne = PolicyContext {
        hour_of_day: Some(11),
        ..Default::default()
    };
    assert!(
        policy.matches(&context_ne),
        "ne operator should match different value"
    );

    let context_eq = PolicyContext {
        hour_of_day: Some(10),
        ..Default::default()
    };
    assert!(
        !policy.matches(&context_eq),
        "ne operator should not match equal value"
    );
}

#[test]
fn test_condition_operator_gt() {
    let mut policy = RoutingPolicy::new("test", "Test Policy");
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TokenCount,
        value: "1000".to_string(),
        operator: "gt".to_string(),
    });

    // Numeric comparison: 2000 > 1000
    let context_gt = PolicyContext {
        token_count: Some(2000),
        ..Default::default()
    };
    assert!(
        policy.matches(&context_gt),
        "gt operator should match greater value"
    );

    let context_eq = PolicyContext {
        token_count: Some(1000),
        ..Default::default()
    };
    assert!(
        !policy.matches(&context_eq),
        "gt operator should not match equal value"
    );

    // Numeric comparison: 999 < 1000, so should not match "gt"
    let context_lt = PolicyContext {
        token_count: Some(999),
        ..Default::default()
    };
    assert!(
        !policy.matches(&context_lt),
        "gt operator should not match lesser value"
    );
}

#[test]
fn test_condition_operator_gte() {
    let mut policy = RoutingPolicy::new("test", "Test Policy");
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TokenCount,
        value: "1000".to_string(),
        operator: "gte".to_string(),
    });

    let context_gt = PolicyContext {
        token_count: Some(2000), // "2000" >= "1000"
        ..Default::default()
    };
    assert!(
        policy.matches(&context_gt),
        "gte operator should match greater value"
    );

    let context_eq = PolicyContext {
        token_count: Some(1000),
        ..Default::default()
    };
    assert!(
        policy.matches(&context_eq),
        "gte operator should match equal value"
    );

    let mut policy2 = RoutingPolicy::new("test2", "Test Policy 2");
    policy2.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TokenCount,
        value: "2000".to_string(),
        operator: "gte".to_string(),
    });

    let context_lt = PolicyContext {
        token_count: Some(1000),
        ..Default::default()
    };
    assert!(
        !policy2.matches(&context_lt),
        "gte operator should not match lesser value"
    );
}

#[test]
fn test_condition_operator_lt() {
    let mut policy = RoutingPolicy::new("test", "Test Policy");
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TokenCount,
        value: "1000".to_string(),
        operator: "lt".to_string(),
    });

    // Numeric comparison: 999 < 1000
    let context_lt = PolicyContext {
        token_count: Some(999),
        ..Default::default()
    };
    assert!(
        policy.matches(&context_lt),
        "lt operator should match lesser value"
    );

    let context_eq = PolicyContext {
        token_count: Some(1000),
        ..Default::default()
    };
    assert!(
        !policy.matches(&context_eq),
        "lt operator should not match equal value"
    );

    let context_gt = PolicyContext {
        token_count: Some(2000),
        ..Default::default()
    };
    assert!(
        !policy.matches(&context_gt),
        "lt operator should not match greater value"
    );
}

#[test]
fn test_condition_operator_lte() {
    let mut policy = RoutingPolicy::new("test", "Test Policy");
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TokenCount,
        value: "2000".to_string(),
        operator: "lte".to_string(),
    });

    let context_lt = PolicyContext {
        token_count: Some(1000),
        ..Default::default()
    };
    assert!(
        policy.matches(&context_lt),
        "lte operator should match lesser value"
    );

    let context_eq = PolicyContext {
        token_count: Some(2000),
        ..Default::default()
    };
    assert!(
        policy.matches(&context_eq),
        "lte operator should match equal value"
    );

    let context_gt = PolicyContext {
        token_count: Some(3000),
        ..Default::default()
    };
    assert!(
        !policy.matches(&context_gt),
        "lte operator should not match greater value"
    );
}

#[test]
fn test_condition_operator_contains() {
    let mut policy = RoutingPolicy::new("test", "Test Policy");
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TenantId,
        value: "admin".to_string(),
        operator: "contains".to_string(),
    });

    let context_contains = PolicyContext {
        tenant_id: Some("super-admin-user".to_string()),
        ..Default::default()
    };
    assert!(
        policy.matches(&context_contains),
        "contains operator should match substring"
    );

    let context_not_contains = PolicyContext {
        tenant_id: Some("regular-user".to_string()),
        ..Default::default()
    };
    assert!(
        !policy.matches(&context_not_contains),
        "contains operator should not match missing substring"
    );
}

#[test]
fn test_condition_operator_in() {
    let mut policy = RoutingPolicy::new("test", "Test Policy");
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TimeOfDay,
        value: "9,10,11,12,13,14,15,16,17".to_string(), // Work hours
        operator: "in".to_string(),
    });

    let context_in = PolicyContext {
        hour_of_day: Some(10),
        ..Default::default()
    };
    assert!(
        policy.matches(&context_in),
        "in operator should match value in list"
    );

    let context_not_in = PolicyContext {
        hour_of_day: Some(22),
        ..Default::default()
    };
    assert!(
        !policy.matches(&context_not_in),
        "in operator should not match value not in list"
    );
}

#[test]
fn test_condition_operator_alternate_syntax() {
    // Test == as alternative to eq
    let mut policy = RoutingPolicy::new("test", "Test Policy");
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TimeOfDay,
        value: "10".to_string(),
        operator: "==".to_string(),
    });

    let context = PolicyContext {
        hour_of_day: Some(10),
        ..Default::default()
    };
    assert!(policy.matches(&context), "== should work as eq alias");

    // Test != as alternative to ne
    policy.conditions[0].operator = "!=".to_string();
    let context_ne = PolicyContext {
        hour_of_day: Some(11),
        ..Default::default()
    };
    assert!(policy.matches(&context_ne), "!= should work as ne alias");

    // Test >= as alternative to gte
    policy.conditions[0].operator = ">=".to_string();
    policy.conditions[0].value = "10".to_string();
    let context_gte = PolicyContext {
        hour_of_day: Some(10),
        ..Default::default()
    };
    assert!(policy.matches(&context_gte), ">= should work as gte alias");

    // Test <= as alternative to lte
    policy.conditions[0].operator = "<=".to_string();
    let context_lte = PolicyContext {
        hour_of_day: Some(10),
        ..Default::default()
    };
    assert!(policy.matches(&context_lte), "<= should work as lte alias");
}

#[test]
fn test_condition_unknown_operator() {
    let mut policy = RoutingPolicy::new("test", "Test Policy");
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TimeOfDay,
        value: "10".to_string(),
        operator: "unknown".to_string(),
    });

    let context = PolicyContext {
        hour_of_day: Some(10),
        ..Default::default()
    };
    assert!(
        !policy.matches(&context),
        "Unknown operator should return false"
    );
}

// ========================================
// PolicyMatcher Edge Cases
// ========================================

#[test]
fn test_matcher_evaluate_empty_registry() {
    let matcher = PolicyMatcher::empty();
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let policies = matcher.evaluate(&model, &context);
    assert!(
        policies.is_empty(),
        "Empty registry should return no matches"
    );
}

#[test]
fn test_matcher_evaluate_disabled_policy() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("disabled", "Disabled Policy").with_priority(10);
    policy.enabled = false;
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let policies = matcher.evaluate(&model, &context);
    assert!(policies.is_empty(), "Disabled policy should not match");
}

#[test]
fn test_matcher_evaluate_complex_conditions() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("complex", "Complex Policy").with_priority(10);

    // Multiple conditions: work hours AND specific tenant
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TimeOfDay,
        value: "9,10,11,12,13,14,15,16,17".to_string(),
        operator: "in".to_string(),
    });
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TenantId,
        value: "premium".to_string(),
        operator: "contains".to_string(),
    });

    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);

    // Both conditions met
    let context_both = PolicyContext {
        hour_of_day: Some(10),
        tenant_id: Some("premium-user".to_string()),
        ..Default::default()
    };
    let policies = matcher.evaluate(&model, &context_both);
    assert!(
        !policies.is_empty(),
        "Should match when both conditions met"
    );

    // Only first condition met
    let context_first = PolicyContext {
        hour_of_day: Some(10),
        tenant_id: Some("regular-user".to_string()),
        ..Default::default()
    };
    let policies = matcher.evaluate(&model, &context_first);
    assert!(
        policies.is_empty(),
        "Should not match when only first condition met"
    );

    // Only second condition met
    let context_second = PolicyContext {
        hour_of_day: Some(22),
        tenant_id: Some("premium-user".to_string()),
        ..Default::default()
    };
    let policies = matcher.evaluate(&model, &context_second);
    assert!(
        policies.is_empty(),
        "Should not match when only second condition met"
    );
}

#[test]
fn test_matcher_evaluate_best_no_matches() {
    let matcher = PolicyMatcher::empty();
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let best = matcher.evaluate_best(&model, &context);
    assert!(
        best.is_none(),
        "Empty registry should return None for best match"
    );
}

#[test]
fn test_matcher_evaluate_best_priority_conflicts() {
    let mut registry = PolicyRegistry::new();

    // Low priority policy
    let low_policy = RoutingPolicy::new("low", "Low Priority")
        .with_priority(10)
        .with_action("prefer");
    registry.add(low_policy);

    // High priority policy
    let high_policy = RoutingPolicy::new("high", "High Priority")
        .with_priority(100)
        .with_action("prefer");
    registry.add(high_policy);

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let best = matcher.evaluate_best(&model, &context);
    assert!(best.is_some());
    assert_eq!(
        best.unwrap().policy.id,
        "high",
        "Should return highest priority policy"
    );
}

#[test]
fn test_matcher_calculate_weight_factor_no_policies() {
    let matcher = PolicyMatcher::empty();
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let factor = matcher.calculate_weight_factor(&model, &context);
    assert!(
        (factor - 1.0).abs() < 0.001,
        "No policies should return neutral weight 1.0"
    );
}

#[test]
fn test_matcher_calculate_weight_factor_normalization() {
    let mut registry = PolicyRegistry::new();

    // Policy with very high weight
    let high_weight_policy = RoutingPolicy::new("high", "High Weight")
        .with_priority(100)
        .with_action("weight")
        .with_weight_factor(50.0);
    registry.add(high_weight_policy);

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let factor = matcher.calculate_weight_factor(&model, &context);
    // Should be clamped to max 10.0
    assert!(
        factor <= 10.0,
        "Weight factor should be clamped to max 10.0"
    );
}

#[test]
fn test_matcher_is_blocked_no_block_policies() {
    let mut registry = PolicyRegistry::new();
    registry.add(templates::vision_required()); // Not a block policy

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    assert!(
        !matcher.is_blocked(&model, &context),
        "Non-block policy should not block"
    );
}

#[test]
fn test_matcher_is_blocked_single_block_policy() {
    let mut registry = PolicyRegistry::new();
    let block_policy = RoutingPolicy::new("block_vision", "Block Vision Models")
        .with_priority(100)
        .with_action("block")
        .with_capability(CapabilityCategory::Vision, "require");
    registry.add(block_policy);

    let matcher = PolicyMatcher::new(registry);

    // Vision model should be blocked
    let vision_model = create_test_model("vision-model", "test", 3.0, 200_000);
    let context = PolicyContext::default();
    assert!(
        matcher.is_blocked(&vision_model, &context),
        "Vision model should be blocked"
    );

    // Non-vision model should not be blocked
    let mut text_model = create_test_model("text-model", "test", 3.0, 200_000);
    text_model.capabilities.vision = false;
    assert!(
        !matcher.is_blocked(&text_model, &context),
        "Non-vision model should not be blocked"
    );
}

#[test]
fn test_matcher_is_blocked_multiple_block_policies() {
    let mut registry = PolicyRegistry::new();

    // Block expensive models
    let mut block_expensive = RoutingPolicy::new("block_expensive", "Block Expensive")
        .with_priority(100)
        .with_action("block");
    block_expensive
        .filters
        .costs
        .push(CostCategory::UltraPremium);
    registry.add(block_expensive);

    // Block specific provider
    let mut block_provider = RoutingPolicy::new("block_provider", "Block Provider")
        .with_priority(100)
        .with_action("block");
    block_provider
        .filters
        .providers
        .push(ProviderCategory::OpenAI);
    registry.add(block_provider);

    let matcher = PolicyMatcher::new(registry);
    let context = PolicyContext::default();

    // Ultra premium model should be blocked
    let expensive_model = create_test_model("expensive", "test", 60.0, 200_000);
    assert!(
        matcher.is_blocked(&expensive_model, &context),
        "Ultra premium model should be blocked"
    );

    // OpenAI model should be blocked
    let openai_model = create_test_model("gpt-4", "openai", 30.0, 128_000);
    assert!(
        matcher.is_blocked(&openai_model, &context),
        "OpenAI model should be blocked"
    );

    // Standard model should not be blocked
    let standard_model = create_test_model("test", "test", 3.0, 200_000);
    assert!(
        !matcher.is_blocked(&standard_model, &context),
        "Standard model should not be blocked"
    );
}

// ========================================
// Policy Condition Types
// ========================================

#[test]
fn test_condition_type_day_of_week() {
    let mut policy = RoutingPolicy::new("test", "Test Policy");
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::DayOfWeek,
        value: "1,2,3,4,5".to_string(), // Weekdays
        operator: "in".to_string(),
    });

    let context_weekday = PolicyContext {
        day_of_week: Some(3), // Wednesday
        ..Default::default()
    };
    assert!(policy.matches(&context_weekday), "Should match weekday");

    let context_weekend = PolicyContext {
        day_of_week: Some(0), // Sunday
        ..Default::default()
    };
    assert!(
        !policy.matches(&context_weekend),
        "Should not match weekend"
    );
}

#[test]
fn test_condition_type_model_family() {
    let mut policy = RoutingPolicy::new("test", "Test Policy");
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::ModelFamily,
        value: "test".to_string(),
        operator: "eq".to_string(),
    });

    let context_match = PolicyContext {
        model_family: Some("test".to_string()),
        ..Default::default()
    };
    assert!(policy.matches(&context_match), "Should match model family");

    let context_no_match = PolicyContext {
        model_family: Some("gpt".to_string()),
        ..Default::default()
    };
    assert!(
        !policy.matches(&context_no_match),
        "Should not match different model family"
    );
}

#[test]
fn test_condition_type_custom_metadata() {
    let mut policy = RoutingPolicy::new("test", "Test Policy");
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::Custom,
        value: "environment:production".to_string(),
        operator: "eq".to_string(),
    });

    let mut metadata = std::collections::HashMap::new();
    metadata.insert("environment".to_string(), "production".to_string());

    let context_match = PolicyContext {
        metadata,
        ..Default::default()
    };
    assert!(
        !policy.matches(&context_match),
        "Custom metadata extracts value but compares to full condition value"
    );

    let mut policy_contains = RoutingPolicy::new("test", "Test Policy");
    policy_contains.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::Custom,
        value: "production".to_string(),
        operator: "contains".to_string(),
    });

    let mut metadata2 = std::collections::HashMap::new();
    metadata2.insert("environment".to_string(), "production".to_string());

    let context_contains = PolicyContext {
        metadata: metadata2,
        ..Default::default()
    };
    assert!(
        !policy_contains.matches(&context_contains),
        "Value without colon returns None for Custom type"
    );
}

#[test]
fn test_condition_missing_context_value() {
    let mut policy = RoutingPolicy::new("test", "Test Policy");
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TenantId,
        value: "admin".to_string(),
        operator: "eq".to_string(),
    });

    let context_no_tenant = PolicyContext {
        tenant_id: None,
        ..Default::default()
    };
    assert!(
        !policy.matches(&context_no_tenant),
        "Should not match when context value is None"
    );
}

// ========================================
// Additional PolicyMatcher Tests — Context Conditions
// ========================================

#[test]
fn test_matcher_evaluate_with_time_of_day_condition() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("work_hours", "Work Hours").with_priority(10);
    // Use "in" operator for string-based comparison of hour values
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TimeOfDay,
        value: "9,10,11,12,13,14,15,16,17".to_string(),
        operator: "in".to_string(),
    });
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);

    // Within work hours (14 is in the list)
    let ctx_work = PolicyContext {
        hour_of_day: Some(14),
        ..Default::default()
    };
    let policies = matcher.evaluate(&model, &ctx_work);
    assert_eq!(policies.len(), 1, "should match during work hours");

    // Outside work hours (22 is not in the list)
    let ctx_off = PolicyContext {
        hour_of_day: Some(22),
        ..Default::default()
    };
    let policies = matcher.evaluate(&model, &ctx_off);
    assert!(policies.is_empty(), "should not match outside work hours");
}

#[test]
fn test_matcher_evaluate_with_day_of_week_condition() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("weekday", "Weekday Only").with_priority(10);
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::DayOfWeek,
        value: "1,2,3,4,5".to_string(),
        operator: "in".to_string(),
    });
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);

    // Wednesday (day 3)
    let ctx_weekday = PolicyContext {
        day_of_week: Some(3),
        ..Default::default()
    };
    let policies = matcher.evaluate(&model, &ctx_weekday);
    assert_eq!(policies.len(), 1, "should match on weekday");

    // Sunday (day 0)
    let ctx_weekend = PolicyContext {
        day_of_week: Some(0),
        ..Default::default()
    };
    let policies = matcher.evaluate(&model, &ctx_weekend);
    assert!(policies.is_empty(), "should not match on weekend");
}

#[test]
fn test_matcher_evaluate_with_token_count_condition() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("large_req", "Large Request Policy").with_priority(10);
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TokenCount,
        value: "5000".to_string(),
        operator: "gt".to_string(),
    });
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);

    // Large request
    let ctx_large = PolicyContext {
        token_count: Some(10_000),
        ..Default::default()
    };
    let policies = matcher.evaluate(&model, &ctx_large);
    assert_eq!(policies.len(), 1, "should match for large token count");

    // Small request
    let ctx_small = PolicyContext {
        token_count: Some(100),
        ..Default::default()
    };
    let policies = matcher.evaluate(&model, &ctx_small);
    assert!(
        policies.is_empty(),
        "should not match for small token count"
    );
}

#[test]
fn test_matcher_evaluate_no_conditions_always_matches() {
    let mut registry = PolicyRegistry::new();
    // Policy with no conditions and no filters — matches everything
    registry.add(RoutingPolicy::new("catch_all", "Catch All").with_priority(1));

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let policies = matcher.evaluate(&model, &context);
    assert_eq!(
        policies.len(),
        1,
        "policy with no conditions should always match"
    );
}

// ========================================
// Multiple Policies — Priority Ordering
// ========================================

#[test]
fn test_matcher_evaluate_best_highest_priority_wins() {
    let mut registry = PolicyRegistry::new();

    registry.add(
        RoutingPolicy::new("low", "Low")
            .with_priority(1)
            .with_action("prefer"),
    );
    registry.add(
        RoutingPolicy::new("mid", "Mid")
            .with_priority(50)
            .with_action("prefer"),
    );
    registry.add(
        RoutingPolicy::new("high", "High")
            .with_priority(100)
            .with_action("prefer"),
    );

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let best = matcher
        .evaluate_best(&model, &context)
        .expect("should have a match");
    assert_eq!(best.policy.id, "high", "highest priority policy should win");
}

#[test]
fn test_matcher_disabled_policies_are_skipped() {
    let mut registry = PolicyRegistry::new();

    let mut disabled = RoutingPolicy::new("disabled", "Disabled").with_priority(100);
    disabled.enabled = false;
    registry.add(disabled);

    registry.add(
        RoutingPolicy::new("active", "Active")
            .with_priority(1)
            .with_action("prefer"),
    );

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let best = matcher
        .evaluate_best(&model, &context)
        .expect("should match active policy");
    assert_eq!(
        best.policy.id, "active",
        "disabled policy should be skipped, active one should match"
    );
}

// ========================================
// Dimension Filter Tests — context_windows, costs, modalities
// ========================================

#[test]
fn test_matcher_context_window_filter_match() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("large_ctx", "Large Context").with_priority(10);
    policy
        .filters
        .context_windows
        .push(ContextWindowCategory::Large);
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);

    // Model with 200K context -> Large category
    let large_model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();
    let policies = matcher.evaluate(&large_model, &context);
    assert_eq!(policies.len(), 1, "large context model should match");

    // Model with 16K context -> Small category
    let small_model = create_test_model("small", "test", 3.0, 16_000);
    let policies = matcher.evaluate(&small_model, &context);
    assert!(
        policies.is_empty(),
        "small context model should not match large filter"
    );
}

#[test]
fn test_matcher_cost_filter_match() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("budget", "Budget Only").with_priority(10);
    policy.filters.costs.push(CostCategory::Economy);
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let context = PolicyContext::default();

    // Economy model (price <= 1.0)
    let economy_model = create_test_model("cheap", "test", 0.5, 100_000);
    let policies = matcher.evaluate(&economy_model, &context);
    assert_eq!(policies.len(), 1, "economy model should match");

    // Standard model (price >= 1.0)
    let standard_model = create_test_model("std", "test", 3.0, 100_000);
    let policies = matcher.evaluate(&standard_model, &context);
    assert!(
        policies.is_empty(),
        "standard cost model should not match economy filter"
    );
}

#[test]
fn test_matcher_modality_image_match() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("image_pol", "Image Required").with_priority(10);
    policy.filters.modalities.push(ModalityCategory::Image);
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let context = PolicyContext::default();

    // Model with vision capability
    let vision_model = create_test_model("vis", "test", 3.0, 200_000);
    let policies = matcher.evaluate(&vision_model, &context);
    assert_eq!(
        policies.len(),
        1,
        "vision model should match image modality"
    );

    // Model without vision capability
    let mut text_model = create_test_model("text", "test", 3.0, 200_000);
    text_model.capabilities.vision = false;
    let policies = matcher.evaluate(&text_model, &context);
    assert!(
        policies.is_empty(),
        "non-vision model should not match image modality"
    );
}

#[test]
fn test_matcher_modality_video_depends_on_vision() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("video_pol", "Video Required").with_priority(10);
    policy.filters.modalities.push(ModalityCategory::Video);
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let context = PolicyContext::default();

    // Vision model should match video modality (inferred from vision capability)
    let vision_model = create_test_model("vis", "test", 3.0, 200_000);
    let policies = matcher.evaluate(&vision_model, &context);
    assert_eq!(
        policies.len(),
        1,
        "vision model should match video modality"
    );

    // Non-vision model should not match
    let mut no_vision = create_test_model("no-vis", "test", 3.0, 200_000);
    no_vision.capabilities.vision = false;
    let policies = matcher.evaluate(&no_vision, &context);
    assert!(
        policies.is_empty(),
        "non-vision model should not match video modality"
    );
}

#[test]
fn test_matcher_modality_audio_never_matches() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("audio_pol", "Audio Required").with_priority(10);
    policy.filters.modalities.push(ModalityCategory::Audio);
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let policies = matcher.evaluate(&model, &context);
    assert!(
        policies.is_empty(),
        "audio modality should never match (not supported by models)"
    );
}

#[test]
fn test_matcher_modality_embedding_never_matches() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("embed_pol", "Embedding Required").with_priority(10);
    policy.filters.modalities.push(ModalityCategory::Embedding);
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let policies = matcher.evaluate(&model, &context);
    assert!(policies.is_empty(), "embedding modality should never match");
}

#[test]
fn test_matcher_modality_text_always_matches() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("text_pol", "Text Required").with_priority(10);
    policy.filters.modalities.push(ModalityCategory::Text);
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let policies = matcher.evaluate(&model, &context);
    assert_eq!(policies.len(), 1, "text modality should always match");
}

#[test]
fn test_matcher_modality_code_always_matches() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("code_pol", "Code Required").with_priority(10);
    policy.filters.modalities.push(ModalityCategory::Code);
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let policies = matcher.evaluate(&model, &context);
    assert_eq!(policies.len(), 1, "code modality should always match");
}

// ========================================
// Capability Exclude Mode
// ========================================

#[test]
fn test_matcher_capability_exclude_mode() {
    let mut registry = PolicyRegistry::new();
    let policy = RoutingPolicy::new("no_vision", "No Vision")
        .with_priority(10)
        .with_capability(CapabilityCategory::Vision, "exclude")
        .with_action("prefer");
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let context = PolicyContext::default();

    // Vision model should be excluded
    let vision_model = create_test_model("vis", "test", 3.0, 200_000);
    let policies = matcher.evaluate(&vision_model, &context);
    assert!(
        policies.is_empty(),
        "vision model should be excluded by exclude mode"
    );

    // Non-vision model should match
    let mut text_model = create_test_model("text", "test", 3.0, 200_000);
    text_model.capabilities.vision = false;
    let policies = matcher.evaluate(&text_model, &context);
    assert_eq!(policies.len(), 1, "non-vision model should match");
}

#[test]
fn test_matcher_capability_thinking_require_and_exclude() {
    let mut registry = PolicyRegistry::new();

    // Require thinking
    let require_thinking = RoutingPolicy::new("req_think", "Require Thinking")
        .with_priority(10)
        .with_capability(CapabilityCategory::Thinking, "require")
        .with_action("prefer");
    registry.add(require_thinking);

    let matcher = PolicyMatcher::new(registry);
    let context = PolicyContext::default();

    // Model without thinking should not match "require"
    let no_thinking = create_test_model("no-think", "test", 3.0, 200_000);
    let policies = matcher.evaluate(&no_thinking, &context);
    assert!(
        policies.is_empty(),
        "model without thinking should not match require filter"
    );

    // Model with thinking should match
    let mut has_thinking = create_test_model("thinker", "test", 3.0, 200_000);
    has_thinking.capabilities.thinking = true;
    let policies = matcher.evaluate(&has_thinking, &context);
    assert_eq!(policies.len(), 1, "thinking model should match require");
}

#[test]
fn test_matcher_capability_streaming_require() {
    let mut registry = PolicyRegistry::new();
    let policy = RoutingPolicy::new("stream_req", "Streaming Required")
        .with_priority(10)
        .with_capability(CapabilityCategory::Streaming, "require")
        .with_action("prefer");
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let context = PolicyContext::default();

    // Default test model has streaming=true
    let streaming_model = create_test_model("test", "test", 3.0, 200_000);
    let policies = matcher.evaluate(&streaming_model, &context);
    assert_eq!(policies.len(), 1, "streaming model should match require");

    // Disable streaming
    let mut no_stream = create_test_model("no-stream", "test", 3.0, 200_000);
    no_stream.capabilities.streaming = false;
    let policies = matcher.evaluate(&no_stream, &context);
    assert!(
        policies.is_empty(),
        "non-streaming model should not match require"
    );
}

#[test]
fn test_matcher_capability_unknown_mode_does_not_filter() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("weird_mode", "Weird Mode").with_priority(10);
    policy.filters.capabilities.push(CapabilityFilter {
        capability: CapabilityCategory::Vision,
        mode: "bogus_mode".to_string(),
    });
    policy.action.action_type = "prefer".to_string();
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let context = PolicyContext::default();

    // Unknown mode should not cause filtering (neither require nor exclude)
    let vision_model = create_test_model("vis", "test", 3.0, 200_000);
    let policies = matcher.evaluate(&vision_model, &context);
    assert_eq!(
        policies.len(),
        1,
        "unknown capability mode should not filter"
    );

    let mut no_vision = create_test_model("no-vis", "test", 3.0, 200_000);
    no_vision.capabilities.vision = false;
    let policies = matcher.evaluate(&no_vision, &context);
    assert_eq!(
        policies.len(),
        1,
        "unknown capability mode should not filter on either side"
    );
}

// ========================================
// Action Constraint Tests
// ========================================

#[test]
fn test_matcher_action_max_cost_per_million() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("budget_cap", "Budget Cap").with_priority(10);
    policy.action.action_type = "prefer".to_string();
    policy.action.max_cost_per_million = Some(5.0);
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let context = PolicyContext::default();

    // Model under the cost cap
    let cheap = create_test_model("cheap", "test", 3.0, 100_000);
    let policies = matcher.evaluate(&cheap, &context);
    assert_eq!(policies.len(), 1, "model under cost cap should match");

    // Model over the cost cap
    let expensive = create_test_model("expensive", "test", 10.0, 100_000);
    let policies = matcher.evaluate(&expensive, &context);
    assert!(policies.is_empty(), "model over cost cap should not match");
}

#[test]
fn test_matcher_action_min_context_window() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("big_ctx", "Big Context").with_priority(10);
    policy.action.action_type = "prefer".to_string();
    policy.action.min_context_window = Some(128_000);
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let context = PolicyContext::default();

    // Model meeting the minimum
    let large = create_test_model("large", "test", 3.0, 200_000);
    let policies = matcher.evaluate(&large, &context);
    assert_eq!(policies.len(), 1, "model meeting min context should match");

    // Model below the minimum
    let small = create_test_model("small", "test", 3.0, 64_000);
    let policies = matcher.evaluate(&small, &context);
    assert!(
        policies.is_empty(),
        "model below min context should not match"
    );
}

#[test]
fn test_matcher_action_avoid_model_id() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("avoid_old", "Avoid Old Models").with_priority(10);
    policy.action.action_type = "prefer".to_string();
    policy.action.avoid.push("gpt-3.5".to_string());
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let context = PolicyContext::default();

    // Model matching avoid pattern
    let avoided = create_test_model("gpt-3.5-turbo", "test", 3.0, 100_000);
    let policies = matcher.evaluate(&avoided, &context);
    assert!(
        policies.is_empty(),
        "model matching avoid pattern should not match"
    );

    // Model not matching avoid pattern
    let safe = create_test_model("gpt-4", "test", 3.0, 100_000);
    let policies = matcher.evaluate(&safe, &context);
    assert_eq!(policies.len(), 1, "safe model should match");
}

#[test]
fn test_matcher_action_avoid_provider() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("avoid_prov", "Avoid Provider").with_priority(10);
    policy.action.action_type = "prefer".to_string();
    policy.action.avoid.push("untrusted".to_string());
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let context = PolicyContext::default();

    // Provider matching avoid pattern
    let avoided = create_test_model("model-1", "untrusted", 3.0, 100_000);
    let policies = matcher.evaluate(&avoided, &context);
    assert!(
        policies.is_empty(),
        "model from avoided provider should not match"
    );

    // Safe provider
    let safe = create_test_model("model-2", "trusted", 3.0, 100_000);
    let policies = matcher.evaluate(&safe, &context);
    assert_eq!(policies.len(), 1, "model from safe provider should match");
}

// ========================================
// Score Calculation Tests
// ========================================

#[test]
fn test_matcher_score_prefer_action_type() {
    let mut registry = PolicyRegistry::new();
    registry.add(
        RoutingPolicy::new("pref", "Prefer")
            .with_priority(10)
            .with_action("prefer"),
    );

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let policies = matcher.evaluate(&model, &context);
    assert_eq!(policies.len(), 1);
    // "prefer" action base score = 1.5, scaled by priority factor (1.0 + 10*0.01 = 1.10)
    let expected_score = 1.5 * 1.10;
    assert!(
        (policies[0].score - expected_score).abs() < 0.001,
        "prefer score should be ~{expected_score:.3}, got {:.3}",
        policies[0].score
    );
}

#[test]
fn test_matcher_score_avoid_action_type() {
    let mut registry = PolicyRegistry::new();
    registry.add(
        RoutingPolicy::new("avoid", "Avoid")
            .with_priority(5)
            .with_action("avoid"),
    );

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let policies = matcher.evaluate(&model, &context);
    assert_eq!(policies.len(), 1);
    // "avoid" action base score = 0.5, priority factor = 1.0 + 5*0.01 = 1.05
    let expected_score = 0.5 * 1.05;
    assert!(
        (policies[0].score - expected_score).abs() < 0.001,
        "avoid score should be ~{expected_score:.3}, got {:.3}",
        policies[0].score
    );
}

#[test]
fn test_matcher_score_block_action_type() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("block_pol", "Block").with_priority(10);
    policy.action.action_type = "block".to_string();
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let policies = matcher.evaluate(&model, &context);
    // "block" action type is not excluded from evaluate, but score is 0.0
    assert_eq!(policies.len(), 1);
    // Base score for "block" = 0.0
    let expected_score = 0.0 * (1.0 + 10.0_f64.mul_add(0.01, 1.0));
    assert!(
        (policies[0].score - expected_score).abs() < 0.001,
        "block score should be ~{expected_score:.3}, got {:.3}",
        policies[0].score
    );
}

#[test]
fn test_matcher_score_weight_action_type_uses_weight_factor() {
    let mut registry = PolicyRegistry::new();
    registry.add(
        RoutingPolicy::new("weight_pol", "Weight")
            .with_priority(10)
            .with_action("weight")
            .with_weight_factor(2.5),
    );

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let policies = matcher.evaluate(&model, &context);
    assert_eq!(policies.len(), 1);
    // "weight" uses weight_factor as base score = 2.5, priority factor = 1.10
    let expected_score = 2.5 * 1.10;
    assert!(
        (policies[0].score - expected_score).abs() < 0.001,
        "weight score should be ~{expected_score:.3}, got {:.3}",
        policies[0].score
    );
}

#[test]
fn test_matcher_score_unknown_action_type_defaults_to_one() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("unknown_action", "Unknown").with_priority(0);
    policy.action.action_type = "teleport".to_string();
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let policies = matcher.evaluate(&model, &context);
    assert_eq!(policies.len(), 1);
    // Unknown action type => base score = 1.0, priority factor = 1.0 + 0*0.01 = 1.0
    let expected_score = 1.0;
    assert!(
        (policies[0].score - expected_score).abs() < 0.001,
        "unknown action score should be ~{expected_score:.3}, got {:.3}",
        policies[0].score
    );
}

#[test]
fn test_matcher_score_preferred_provider_bonus() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("pref_openai", "Prefer OpenAI").with_priority(0);
    policy.action.action_type = "prefer".to_string();
    policy
        .action
        .preferred_providers
        .push(ProviderCategory::OpenAI);
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let context = PolicyContext::default();

    // OpenAI model gets bonus
    let openai_model = create_test_model("gpt-4", "openai", 30.0, 128_000);
    let policies = matcher.evaluate(&openai_model, &context);
    assert_eq!(policies.len(), 1);
    // base=1.5, preferred_provider bonus=1.2x, priority_factor=1.0
    let expected_score = 1.5 * 1.2 * 1.0;
    assert!(
        (policies[0].score - expected_score).abs() < 0.001,
        "preferred provider score should be ~{expected_score:.3}, got {:.3}",
        policies[0].score
    );

    // Non-OpenAI model does not get bonus
    let other_model = create_test_model("test", "anthropic", 3.0, 200_000);
    let policies = matcher.evaluate(&other_model, &context);
    assert_eq!(policies.len(), 1);
    let expected_no_bonus = 1.5 * 1.0;
    assert!(
        (policies[0].score - expected_no_bonus).abs() < 0.001,
        "non-preferred provider score should be ~{expected_no_bonus:.3}, got {:.3}",
        policies[0].score
    );
}

#[test]
fn test_matcher_score_preferred_model_bonus() {
    let mut registry = PolicyRegistry::new();
    let mut policy = RoutingPolicy::new("pref_model", "Prefer Specific").with_priority(0);
    policy.action.action_type = "prefer".to_string();
    policy.action.preferred_models.push("claude".to_string());
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let context = PolicyContext::default();

    // Model ID contains "claude" — gets bonus
    let claude_model = create_test_model("claude-sonnet-4", "anthropic", 3.0, 200_000);
    let policies = matcher.evaluate(&claude_model, &context);
    assert_eq!(policies.len(), 1);
    // base=1.5, preferred_model bonus=1.3x, priority_factor=1.0
    let expected_score = 1.5 * 1.3;
    assert!(
        (policies[0].score - expected_score).abs() < 0.001,
        "preferred model score should be ~{expected_score:.3}, got {:.3}",
        policies[0].score
    );
}

#[test]
fn test_matcher_score_prefer_capability_bonus() {
    let mut registry = PolicyRegistry::new();
    let policy = RoutingPolicy::new("pref_vision", "Prefer Vision")
        .with_priority(0)
        .with_capability(CapabilityCategory::Vision, "prefer")
        .with_action("prefer");
    registry.add(policy);

    let matcher = PolicyMatcher::new(registry);
    let context = PolicyContext::default();

    // Vision model gets the prefer bonus
    let vision_model = create_test_model("vis", "test", 3.0, 200_000);
    let policies = matcher.evaluate(&vision_model, &context);
    assert_eq!(policies.len(), 1);
    // base=1.5, prefer cap bonus=1.1x, priority_factor=1.0
    let expected_score = 1.5 * 1.1;
    assert!(
        (policies[0].score - expected_score).abs() < 0.001,
        "prefer capability bonus score should be ~{expected_score:.3}, got {:.3}",
        policies[0].score
    );

    // Non-vision model does not get the prefer bonus
    let mut no_vision = create_test_model("no-vis", "test", 3.0, 200_000);
    no_vision.capabilities.vision = false;
    let policies = matcher.evaluate(&no_vision, &context);
    assert_eq!(policies.len(), 1);
    let expected_no_bonus = 1.5;
    assert!(
        (policies[0].score - expected_no_bonus).abs() < 0.001,
        "non-vision prefer score should be ~{expected_no_bonus:.3}, got {:.3}",
        policies[0].score
    );
}

// ========================================
// Weight Factor Edge Cases
// ========================================

#[test]
fn test_matcher_weight_factor_with_zero_priority_policies() {
    let mut registry = PolicyRegistry::new();
    registry.add(
        RoutingPolicy::new("zero", "Zero Priority")
            .with_priority(0)
            .with_action("prefer"),
    );

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    let factor = matcher.calculate_weight_factor(&model, &context);
    // Should not panic or return NaN, should be in [0.1, 10.0]
    assert!(
        factor.is_finite(),
        "weight factor should be finite, got {factor}"
    );
    assert!(
        (0.1..=10.0).contains(&factor),
        "weight factor should be in [0.1, 10.0], got {factor}"
    );
}

// ========================================
// Is Blocked Edge Cases
// ========================================

#[test]
fn test_matcher_is_blocked_disabled_block_policy_does_not_block() {
    let mut registry = PolicyRegistry::new();
    let mut block = RoutingPolicy::new("disabled_block", "Disabled Block")
        .with_priority(100)
        .with_action("block");
    block.enabled = false;
    registry.add(block);

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    assert!(
        !matcher.is_blocked(&model, &context),
        "disabled block policy should not block"
    );
}

#[test]
fn test_matcher_is_blocked_non_block_action_does_not_block() {
    let mut registry = PolicyRegistry::new();
    registry.add(
        RoutingPolicy::new("prefer_pol", "Prefer")
            .with_priority(100)
            .with_action("prefer"),
    );

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();

    assert!(
        !matcher.is_blocked(&model, &context),
        "prefer policy should not block"
    );
}

// ========================================
// PolicyMatcher Clone and Accessors
// ========================================

#[test]
fn test_matcher_clone_preserves_policies() {
    let mut registry = PolicyRegistry::new();
    registry.add(RoutingPolicy::new("p1", "Policy 1").with_priority(10));

    let matcher = PolicyMatcher::new(registry);

    // Verify original has the policy
    assert_eq!(
        matcher.registry().all().len(),
        1,
        "original matcher should have the policy"
    );

    // Clone and verify the clone also has the policy
    let cloned = matcher.clone();

    assert_eq!(
        cloned.registry().all().len(),
        1,
        "cloned matcher should have same policies"
    );
    assert_eq!(
        cloned.registry().all()[0].id,
        "p1",
        "cloned matcher should preserve policy data"
    );

    // Verify both matchers produce the same results
    let model = create_test_model("test", "test", 3.0, 200_000);
    let context = PolicyContext::default();
    let original_matches = matcher.evaluate(&model, &context);
    let cloned_matches = cloned.evaluate(&model, &context);
    assert_eq!(
        original_matches.len(),
        cloned_matches.len(),
        "original and cloned matcher should produce identical results"
    );
}

#[test]
fn test_matcher_empty_factory() {
    let matcher = PolicyMatcher::empty();
    assert_eq!(
        matcher.registry().all().len(),
        0,
        "empty matcher should have no policies"
    );
}

#[test]
fn test_matcher_registry_mut_allows_adding_policies() {
    let mut matcher = PolicyMatcher::empty();
    matcher
        .registry_mut()
        .add(RoutingPolicy::new("dynamic", "Dynamic").with_priority(10));

    assert_eq!(
        matcher.registry().all().len(),
        1,
        "registry_mut should allow adding policies"
    );
    assert_eq!(
        matcher.registry().all()[0].id,
        "dynamic",
        "added policy should be accessible"
    );
}
