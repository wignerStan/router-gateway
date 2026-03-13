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

// ========================================
// Schema Validation Tests
// ========================================

#[test]
fn test_validate_schema_valid_policies_json() {
    let schema = PolicyRegistry::load_schema();
    let json = r#"{
        "policies": [
            {
                "id": "test_policy",
                "name": "Test Policy",
                "priority": 10,
                "enabled": true,
                "filters": {},
                "action": {"action_type": "prefer", "weight_factor": 1.5},
                "conditions": []
            }
        ]
    }"#;
    let result = PolicyRegistry::validate_against_schema(json, &schema);
    assert!(
        result.is_ok(),
        "Valid policies JSON should pass schema validation: {result:?}"
    );
}

#[test]
fn test_validate_schema_missing_required_field() {
    let schema = PolicyRegistry::load_schema();
    let json = r#"{
        "policies": [
            {
                "id": "no_name_policy",
                "priority": 10,
                "enabled": true,
                "filters": {},
                "action": {"action_type": "prefer"}
            }
        ]
    }"#;
    let result = PolicyRegistry::validate_against_schema(json, &schema);
    assert!(
        result.is_err(),
        "Policy without required 'name' field should fail schema validation"
    );
}

#[test]
fn test_validate_schema_invalid_action_type() {
    let schema = PolicyRegistry::load_schema();
    let json = r#"{
        "policies": [
            {
                "id": "bad_action",
                "name": "Bad Action",
                "priority": 10,
                "enabled": true,
                "filters": {},
                "action": {"action_type": "explode", "weight_factor": 1.0},
                "conditions": []
            }
        ]
    }"#;
    let result = PolicyRegistry::validate_against_schema(json, &schema);
    assert!(
        result.is_err(),
        "Invalid action_type should fail schema validation"
    );
}

#[test]
fn test_validate_schema_invalid_capability() {
    let schema = PolicyRegistry::load_schema();
    let json = r#"{
        "policies": [
            {
                "id": "bad_cap",
                "name": "Bad Cap",
                "priority": 10,
                "enabled": true,
                "filters": {
                    "capabilities": [{"capability": "telekinesis", "mode": "require"}]
                },
                "action": {"action_type": "prefer"},
                "conditions": []
            }
        ]
    }"#;
    let result = PolicyRegistry::validate_against_schema(json, &schema);
    assert!(
        result.is_err(),
        "Invalid capability value should fail schema validation"
    );
}

#[test]
fn test_from_file_loads_and_validates_policies_json() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let policies_path = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("config")
        .join("policies.json");

    let registry = PolicyRegistry::from_file(&policies_path);
    assert!(
        registry.is_ok(),
        "config/policies.json should load successfully: {registry:?}"
    );
    let registry = registry.unwrap();
    assert_eq!(
        registry.all().len(),
        10,
        "Should load all 10 policies from config/policies.json"
    );
}

#[test]
fn test_from_file_rejects_invalid_json() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), r#"{"policies": [{"id": 1, "name": "Bad"}]}"#).unwrap();

    let result = PolicyRegistry::from_file(tmp.path());
    assert!(
        result.is_err(),
        "Non-string id should fail schema validation"
    );
}

#[test]
fn test_from_file_rejects_missing_schema_elements() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        tmp.path(),
        r#"{"policies": [{"priority": 5, "enabled": true, "filters": {}, "action": {"action_type": "prefer"}}]}"#,
    )
    .unwrap();

    let result = PolicyRegistry::from_file(tmp.path());
    assert!(
        result.is_err(),
        "Missing required fields should fail validation"
    );
}
