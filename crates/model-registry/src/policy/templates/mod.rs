//! Predefined policy templates for common use cases.

use super::types::*;

/// Create cost-optimization policy
pub fn cost_optimization() -> RoutingPolicy {
    RoutingPolicy::new("cost_optimization", "Cost Optimization")
        .with_priority(10)
        .with_action("weight")
        .with_weight_factor(1.5)
}

/// Create performance-first policy (prefer fast models)
pub fn performance_first() -> RoutingPolicy {
    RoutingPolicy::new("performance_first", "Performance First")
        .with_priority(20)
        .with_tier(TierCategory::Fast)
        .with_action("prefer")
}

/// Create quality-first policy (prefer flagship models)
pub fn quality_first() -> RoutingPolicy {
    RoutingPolicy::new("quality_first", "Quality First")
        .with_priority(20)
        .with_tier(TierCategory::Flagship)
        .with_action("prefer")
}

/// Create vision-capable policy
pub fn vision_required() -> RoutingPolicy {
    RoutingPolicy::new("vision_required", "Vision Required")
        .with_priority(30)
        .with_capability(CapabilityCategory::Vision, "require")
        .with_action("prefer")
}

/// Create thinking-required policy
pub fn thinking_required() -> RoutingPolicy {
    RoutingPolicy::new("thinking_required", "Extended Thinking Required")
        .with_priority(30)
        .with_capability(CapabilityCategory::Thinking, "require")
        .with_action("prefer")
}

/// Create large context policy
pub fn large_context() -> RoutingPolicy {
    RoutingPolicy::new("large_context", "Large Context Required")
        .with_priority(25)
        .with_action("prefer")
}

/// Create provider preference policy
pub fn prefer_provider(provider: ProviderCategory) -> RoutingPolicy {
    RoutingPolicy::new(
        format!("prefer_{:?}", provider).to_lowercase(),
        format!("Prefer {:?}", provider),
    )
    .with_priority(15)
    .with_provider(provider)
    .with_action("prefer")
}

/// Create off-peak hours policy
pub fn off_peak_hours() -> RoutingPolicy {
    let mut policy = RoutingPolicy::new("off_peak_hours", "Off-Peak Hours")
        .with_priority(5)
        .with_action("weight")
        .with_weight_factor(0.8);

    // Off-peak: 22:00 - 06:00
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TimeOfDay,
        value: "22,23,0,1,2,3,4,5,6".to_string(),
        operator: "in".to_string(),
    });

    policy
}
