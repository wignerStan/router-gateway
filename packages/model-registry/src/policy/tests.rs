use super::registry::PolicyRegistry;
use super::templates;
use super::types::*;

#[test]
fn test_policy_creation() {
    let policy = RoutingPolicy::new("test", "Test Policy")
        .with_priority(10)
        .with_capability(CapabilityCategory::Vision, "require")
        .with_tier(TierCategory::Standard);

    assert_eq!(policy.id, "test");
    assert_eq!(policy.priority, 10);
    assert_eq!(policy.filters.capabilities.len(), 1);
    assert_eq!(policy.filters.tiers.len(), 1);
}

#[test]
fn test_policy_matching() {
    let policy = RoutingPolicy::new("test", "Test Policy").with_priority(10);

    let context = PolicyContext::default();
    assert!(policy.matches(&context));

    // Test with condition
    let mut policy_with_condition = policy.clone();
    policy_with_condition.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TimeOfDay,
        value: "10".to_string(),
        operator: "eq".to_string(),
    });

    let context_with_hour = PolicyContext {
        hour_of_day: Some(10),
        ..Default::default()
    };
    assert!(policy_with_condition.matches(&context_with_hour));

    let context_wrong_hour = PolicyContext {
        hour_of_day: Some(11),
        ..Default::default()
    };
    assert!(!policy_with_condition.matches(&context_wrong_hour));
}

#[test]
fn test_policy_registry() {
    let mut registry = PolicyRegistry::new();

    let policy1 = RoutingPolicy::new("p1", "Policy 1").with_priority(10);
    let policy2 = RoutingPolicy::new("p2", "Policy 2").with_priority(20);

    registry.add(policy1);
    registry.add(policy2);

    // Should be sorted by priority (p2 first)
    assert_eq!(registry.all().len(), 2);
    assert_eq!(registry.all()[0].id, "p2");

    // Get by ID
    assert!(registry.get("p1").is_some());

    // Remove
    assert!(registry.remove("p1"));
    assert_eq!(registry.all().len(), 1);
}

#[test]
fn test_modality_category() {
    assert_eq!(ModalityCategory::Text.as_str(), "text");
    assert_eq!(
        ModalityCategory::parse("image"),
        Some(ModalityCategory::Image)
    );
    assert_eq!(ModalityCategory::parse("unknown"), None);
}

#[test]
fn test_policy_templates() {
    let vision_policy = templates::vision_required();
    assert!(vision_policy.enabled);
    assert!(!vision_policy.filters.capabilities.is_empty());

    let perf_policy = templates::performance_first();
    assert_eq!(perf_policy.filters.tiers, vec![TierCategory::Fast]);
}

#[test]
fn test_policy_serialization() {
    let policy = RoutingPolicy::new("test", "Test Policy")
        .with_priority(10)
        .with_capability(CapabilityCategory::Vision, "require");

    let json = serde_json::to_string(&policy).unwrap();
    assert!(json.contains("\"id\":\"test\""));

    let deserialized: RoutingPolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, policy.id);
}
