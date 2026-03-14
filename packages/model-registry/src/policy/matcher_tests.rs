use super::matcher::PolicyMatcher;
use super::registry::PolicyRegistry;
use super::templates;
use super::types::*;
use crate::categories::{CapabilityCategory, CostCategory, ProviderCategory};
use crate::info::{DataSource, ModelCapabilities, ModelInfo, RateLimits};

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
            tokens_per_minute: 90000,
        },
        source: DataSource::Static,
    }
}

#[test]
fn test_matcher_basic_matching() {
    let mut registry = PolicyRegistry::new();
    registry.add(templates::vision_required());

    let matcher = PolicyMatcher::new(registry);
    let model = create_test_model("test", "test", 3.0, 200000);
    let context = PolicyContext::default();

    let matches = matcher.evaluate(&model, &context);
    assert!(
        !matches.is_empty(),
        "Vision policy should match vision-capable model"
    );
}

#[test]
fn test_matcher_tier_filtering() {
    let mut registry = PolicyRegistry::new();
    registry.add(templates::performance_first()); // Fast tier only

    let matcher = PolicyMatcher::new(registry);

    // Fast model (price <= 1.0)
    let fast_model = create_test_model("fast-model", "test", 0.5, 100000);
    let context = PolicyContext::default();

    let matches = matcher.evaluate(&fast_model, &context);
    assert!(
        !matches.is_empty(),
        "Performance policy should match fast model"
    );

    // Flagship model (high price)
    let flagship_model = create_test_model("flagship-model", "test", 20.0, 200000);
    let matches = matcher.evaluate(&flagship_model, &context);
    assert!(
        matches.is_empty(),
        "Performance policy should not match flagship model"
    );
}

#[test]
fn test_matcher_provider_filtering() {
    let mut registry = PolicyRegistry::new();
    registry.add(templates::prefer_provider(ProviderCategory::OpenAI));

    let matcher = PolicyMatcher::new(registry);

    let openai_model = create_test_model("test", "openai", 30.0, 128000);
    let context = PolicyContext::default();

    let matches = matcher.evaluate(&openai_model, &context);
    assert!(!matches.is_empty(), "Should match OpenAI provider");
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
    let model = create_test_model("test", "test", 1.0, 100000);
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
    let expensive_model = create_test_model("expensive", "test", 60.0, 100000);
    let context = PolicyContext::default();
    assert!(
        matcher.is_blocked(&expensive_model, &context),
        "Ultra premium model should be blocked"
    );

    // Standard cost model should not be blocked
    let cheap_model = create_test_model("cheap", "test", 3.0, 100000);
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
    let model = create_test_model("test", "test", 15.0, 200000);
    let context = PolicyContext::default();

    let best = matcher.evaluate_best(&model, &context);
    assert!(best.is_some());

    // Quality first has higher priority, should be selected
    let best = best.expect("value must be present");
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
    let matching_model = create_test_model("test", "openai", 3.0, 200000);
    let context = PolicyContext::default();
    let matches = matcher.evaluate(&matching_model, &context);
    assert!(!matches.is_empty(), "Should match all dimensions");

    // Model missing vision
    let mut non_vision_model = create_test_model("text-only", "openai", 3.0, 200000);
    non_vision_model.capabilities.vision = false;
    let matches = matcher.evaluate(&non_vision_model, &context);
    assert!(matches.is_empty(), "Should not match - no vision");
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
    let model = create_test_model("test", "test", 3.0, 200000);
    let context = PolicyContext::default();

    let matches = matcher.evaluate(&model, &context);
    assert!(
        matches.is_empty(),
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
    let model = create_test_model("test", "test", 3.0, 200000);
    let context = PolicyContext::default();

    let matches = matcher.evaluate(&model, &context);
    assert!(matches.is_empty(), "Disabled policy should not match");
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
    let model = create_test_model("test", "test", 3.0, 200000);

    // Both conditions met
    let context_both = PolicyContext {
        hour_of_day: Some(10),
        tenant_id: Some("premium-user".to_string()),
        ..Default::default()
    };
    let matches = matcher.evaluate(&model, &context_both);
    assert!(!matches.is_empty(), "Should match when both conditions met");

    // Only first condition met
    let context_first = PolicyContext {
        hour_of_day: Some(10),
        tenant_id: Some("regular-user".to_string()),
        ..Default::default()
    };
    let matches = matcher.evaluate(&model, &context_first);
    assert!(
        matches.is_empty(),
        "Should not match when only first condition met"
    );

    // Only second condition met
    let context_second = PolicyContext {
        hour_of_day: Some(22),
        tenant_id: Some("premium-user".to_string()),
        ..Default::default()
    };
    let matches = matcher.evaluate(&model, &context_second);
    assert!(
        matches.is_empty(),
        "Should not match when only second condition met"
    );
}

#[test]
fn test_matcher_evaluate_best_no_matches() {
    let matcher = PolicyMatcher::empty();
    let model = create_test_model("test", "test", 3.0, 200000);
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
    let model = create_test_model("test", "test", 3.0, 200000);
    let context = PolicyContext::default();

    let best = matcher.evaluate_best(&model, &context);
    assert!(best.is_some());
    assert_eq!(
        best.expect("value must be present").policy.id,
        "high",
        "Should return highest priority policy"
    );
}

#[test]
fn test_matcher_calculate_weight_factor_no_policies() {
    let matcher = PolicyMatcher::empty();
    let model = create_test_model("test", "test", 3.0, 200000);
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
    let model = create_test_model("test", "test", 3.0, 200000);
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
    let model = create_test_model("test", "test", 3.0, 200000);
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
    let vision_model = create_test_model("vision-model", "test", 3.0, 200000);
    let context = PolicyContext::default();
    assert!(
        matcher.is_blocked(&vision_model, &context),
        "Vision model should be blocked"
    );

    // Non-vision model should not be blocked
    let mut text_model = create_test_model("text-model", "test", 3.0, 200000);
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
    let expensive_model = create_test_model("expensive", "test", 60.0, 200000);
    assert!(
        matcher.is_blocked(&expensive_model, &context),
        "Ultra premium model should be blocked"
    );

    // OpenAI model should be blocked
    let openai_model = create_test_model("gpt-4", "openai", 30.0, 128000);
    assert!(
        matcher.is_blocked(&openai_model, &context),
        "OpenAI model should be blocked"
    );

    // Standard model should not be blocked
    let standard_model = create_test_model("test", "test", 3.0, 200000);
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
