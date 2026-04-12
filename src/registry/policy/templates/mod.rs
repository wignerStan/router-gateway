//! Predefined policy templates for common routing scenarios.

use super::types::{
    CapabilityCategory, PolicyCondition, PolicyConditionType, ProviderCategory, RoutingPolicy,
    TierCategory,
};

/// Cost-optimization policy: boosts weight for cheaper models.
#[must_use]
pub fn cost_optimization() -> RoutingPolicy {
    RoutingPolicy::new("cost_optimization", "Cost Optimization")
        .with_priority(10)
        .with_action("weight")
        .with_weight_factor(1.5)
}

/// Performance-first policy: prefers fast-tier models.
#[must_use]
pub fn performance_first() -> RoutingPolicy {
    RoutingPolicy::new("performance_first", "Performance First")
        .with_priority(20)
        .with_tier(TierCategory::Fast)
        .with_action("prefer")
}

/// Quality-first policy: prefers flagship-tier models.
#[must_use]
pub fn quality_first() -> RoutingPolicy {
    RoutingPolicy::new("quality_first", "Quality First")
        .with_priority(20)
        .with_tier(TierCategory::Flagship)
        .with_action("prefer")
}

/// Vision-required policy: requires vision capability.
#[must_use]
pub fn vision_required() -> RoutingPolicy {
    RoutingPolicy::new("vision_required", "Vision Required")
        .with_priority(30)
        .with_capability(CapabilityCategory::Vision, "require")
        .with_action("prefer")
}

/// Thinking-required policy: requires extended thinking capability.
#[must_use]
pub fn thinking_required() -> RoutingPolicy {
    RoutingPolicy::new("thinking_required", "Extended Thinking Required")
        .with_priority(30)
        .with_capability(CapabilityCategory::Thinking, "require")
        .with_action("prefer")
}

/// Large-context policy: prefers models with large context windows.
#[must_use]
pub fn large_context() -> RoutingPolicy {
    RoutingPolicy::new("large_context", "Large Context Required")
        .with_priority(25)
        .with_action("prefer")
}

/// Provider preference policy: boosts weight for a specific provider.
#[must_use]
pub fn prefer_provider(provider: ProviderCategory) -> RoutingPolicy {
    RoutingPolicy::new(
        format!("prefer_{provider:?}").to_lowercase(),
        format!("Prefer {provider:?}"),
    )
    .with_priority(15)
    .with_provider(provider)
    .with_action("prefer")
}

/// Off-peak hours policy: reduces weight during low-traffic hours (22:00-06:00).
#[must_use]
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn cost_optimization_has_correct_fields() {
        let p = cost_optimization();
        assert_eq!(p.id, "cost_optimization");
        assert_eq!(p.name, "Cost Optimization");
        assert_eq!(p.priority, 10);
        assert_eq!(p.action.action_type, "weight");
        assert!((p.action.weight_factor - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn performance_first_prefers_fast_tier() {
        let p = performance_first();
        assert_eq!(p.id, "performance_first");
        assert_eq!(p.priority, 20);
        assert_eq!(p.action.action_type, "prefer");
        assert!(p.filters.tiers.contains(&TierCategory::Fast));
    }

    #[test]
    fn quality_first_prefers_flagship_tier() {
        let p = quality_first();
        assert_eq!(p.id, "quality_first");
        assert_eq!(p.priority, 20);
        assert!(p.filters.tiers.contains(&TierCategory::Flagship));
    }

    #[test]
    fn vision_required_requires_vision_capability() {
        let p = vision_required();
        assert_eq!(p.id, "vision_required");
        assert_eq!(p.priority, 30);
        assert!(
            p.filters
                .capabilities
                .iter()
                .any(|c| c.capability == CapabilityCategory::Vision)
        );
    }

    #[test]
    fn thinking_required_requires_thinking_capability() {
        let p = thinking_required();
        assert_eq!(p.id, "thinking_required");
        assert_eq!(p.priority, 30);
        assert!(
            p.filters
                .capabilities
                .iter()
                .any(|c| c.capability == CapabilityCategory::Thinking)
        );
    }

    #[test]
    fn large_context_has_correct_action() {
        let p = large_context();
        assert_eq!(p.id, "large_context");
        assert_eq!(p.priority, 25);
        assert_eq!(p.action.action_type, "prefer");
    }

    #[test]
    fn prefer_provider_sets_provider_filter() {
        let p = prefer_provider(ProviderCategory::OpenAI);
        assert!(p.filters.providers.contains(&ProviderCategory::OpenAI));
        assert_eq!(p.action.action_type, "prefer");
    }

    #[test]
    fn off_peak_hours_has_time_condition() {
        let p = off_peak_hours();
        assert_eq!(p.id, "off_peak_hours");
        assert_eq!(p.action.action_type, "weight");
        assert!((p.action.weight_factor - 0.8).abs() < f64::EPSILON);
        assert!(
            p.conditions
                .iter()
                .any(|c| c.condition_type == PolicyConditionType::TimeOfDay)
        );
    }

    #[test]
    fn all_templates_are_enabled_by_default() {
        let templates: Vec<RoutingPolicy> = vec![
            cost_optimization(),
            performance_first(),
            quality_first(),
            vision_required(),
            thinking_required(),
            large_context(),
            off_peak_hours(),
        ];
        for t in &templates {
            assert!(t.enabled, "Template {} should be enabled", t.id);
        }
    }
}
