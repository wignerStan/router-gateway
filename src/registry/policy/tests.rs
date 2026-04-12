use super::registry::PolicyRegistry;
use super::templates;
use super::types::*;
use pretty_assertions::assert_eq;

// ========================================
// PolicyRegistry Unit Tests
// ========================================

#[test]
fn test_registry_new_creates_empty_registry() {
    let registry = PolicyRegistry::new();
    assert_eq!(
        registry.all().len(),
        0,
        "new registry should have no policies"
    );
}

#[test]
fn test_registry_default_is_empty() {
    let registry = PolicyRegistry::default();
    assert_eq!(
        registry.all().len(),
        0,
        "default registry should have no policies"
    );
}

#[test]
fn test_registry_add_single_policy() {
    let mut registry = PolicyRegistry::new();
    let policy = RoutingPolicy::new("pol-1", "First Policy").with_priority(5);
    registry.add(policy);

    assert_eq!(registry.all().len(), 1);
    assert_eq!(registry.all()[0].id, "pol-1");
    assert_eq!(registry.all()[0].priority, 5);
}

#[test]
fn test_registry_add_sorts_by_priority_descending() {
    let mut registry = PolicyRegistry::new();

    registry.add(RoutingPolicy::new("low", "Low").with_priority(1));
    registry.add(RoutingPolicy::new("high", "High").with_priority(100));
    registry.add(RoutingPolicy::new("mid", "Mid").with_priority(50));

    assert_eq!(registry.all().len(), 3);
    // Highest priority first
    assert_eq!(registry.all()[0].id, "high");
    assert_eq!(registry.all()[1].id, "mid");
    assert_eq!(registry.all()[2].id, "low");
}

#[test]
fn test_registry_get_existing_policy() {
    let mut registry = PolicyRegistry::new();
    registry.add(RoutingPolicy::new("pol-1", "First").with_priority(10));

    let found = registry.get("pol-1").expect("should find existing policy");
    assert_eq!(found.id, "pol-1");
    assert_eq!(found.name, "First");
}

#[test]
fn test_registry_get_nonexistent_returns_none() {
    let mut registry = PolicyRegistry::new();
    registry.add(RoutingPolicy::new("pol-1", "First").with_priority(10));

    assert_eq!(
        registry.get("does-not-exist"),
        None,
        "should return None for missing policy"
    );
}

#[test]
fn test_registry_get_on_empty_returns_none() {
    let registry = PolicyRegistry::new();
    assert_eq!(
        registry.get("anything"),
        None,
        "empty registry should return None for any get"
    );
}

#[test]
fn test_registry_all_returns_all_policies() {
    let mut registry = PolicyRegistry::new();
    registry.add(RoutingPolicy::new("a", "A").with_priority(1));
    registry.add(RoutingPolicy::new("b", "B").with_priority(2));
    registry.add(RoutingPolicy::new("c", "C").with_priority(3));

    let all = registry.all();
    assert_eq!(all.len(), 3);

    let ids: Vec<&str> = all.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"a"));
    assert!(ids.contains(&"b"));
    assert!(ids.contains(&"c"));
}

#[test]
fn test_registry_remove_existing_returns_true() {
    let mut registry = PolicyRegistry::new();
    registry.add(RoutingPolicy::new("pol-1", "First").with_priority(10));

    let removed = registry.remove("pol-1");
    assert!(removed, "remove should return true for existing policy");
    assert_eq!(registry.all().len(), 0);
}

#[test]
fn test_registry_remove_nonexistent_returns_false() {
    let mut registry = PolicyRegistry::new();
    registry.add(RoutingPolicy::new("pol-1", "First").with_priority(10));

    let removed = registry.remove("ghost");
    assert!(
        !removed,
        "remove should return false for nonexistent policy"
    );
    assert_eq!(registry.all().len(), 1, "existing policy should remain");
}

#[test]
fn test_registry_remove_from_empty_returns_false() {
    let mut registry = PolicyRegistry::new();
    let removed = registry.remove("anything");
    assert!(!removed, "remove on empty registry should return false");
}

#[test]
fn test_registry_register_duplicate_id() {
    let mut registry = PolicyRegistry::new();

    registry.add(RoutingPolicy::new("dup", "First").with_priority(5));
    registry.add(RoutingPolicy::new("dup", "Second").with_priority(10));

    // Both are stored (no deduplication in add)
    assert_eq!(registry.all().len(), 2);

    // get returns the first match (higher priority, so "Second")
    let found = registry.get("dup").expect("should find policy");
    assert_eq!(found.name, "Second");
}

#[test]
fn test_registry_find_matches_no_conditions() {
    let mut registry = PolicyRegistry::new();
    registry.add(RoutingPolicy::new("p1", "P1").with_priority(10));
    registry.add(RoutingPolicy::new("p2", "P2").with_priority(20));

    let context = PolicyContext::default();
    let matches = registry.find_matches(&context);
    assert_eq!(
        matches.len(),
        2,
        "all enabled policies with no conditions should match"
    );
}

#[test]
fn test_registry_find_matches_with_conditions() {
    let mut registry = PolicyRegistry::new();

    let mut hour_policy = RoutingPolicy::new("hour_pol", "Hour Policy").with_priority(10);
    hour_policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TimeOfDay,
        value: "14".to_string(),
        operator: "eq".to_string(),
    });
    registry.add(hour_policy);

    registry.add(RoutingPolicy::new("uncond_pol", "Unconditional").with_priority(5));

    // Matching context: hour matches
    let ctx_match = PolicyContext {
        hour_of_day: Some(14),
        ..Default::default()
    };
    let matches = registry.find_matches(&ctx_match);
    assert_eq!(matches.len(), 2, "both policies should match");

    // Non-matching context: hour does not match
    let ctx_no_match = PolicyContext {
        hour_of_day: Some(3),
        ..Default::default()
    };
    let matches = registry.find_matches(&ctx_no_match);
    assert_eq!(matches.len(), 1, "only unconditional policy should match");
    assert_eq!(matches[0].id, "uncond_pol");
}

#[test]
fn test_registry_find_matches_skips_disabled() {
    let mut registry = PolicyRegistry::new();
    let mut disabled = RoutingPolicy::new("disabled", "Disabled").with_priority(10);
    disabled.enabled = false;
    registry.add(disabled);

    let context = PolicyContext::default();
    let matches = registry.find_matches(&context);
    assert!(matches.is_empty(), "disabled policy should not match");
}

#[test]
fn test_registry_from_json_valid_array() {
    let json = r#"[
        {"id":"p1","name":"First","priority":10,"enabled":true,"filters":{},"action":{"action_type":"prefer","weight_factor":1.0},"conditions":[]},
        {"id":"p2","name":"Second","priority":20,"enabled":true,"filters":{},"action":{"action_type":"avoid","weight_factor":1.0},"conditions":[]}
    ]"#;

    let registry = PolicyRegistry::from_json(json).expect("valid JSON should parse");
    assert_eq!(registry.all().len(), 2);
    // Sorted by priority: p2 first
    assert_eq!(registry.all()[0].id, "p2");
    assert_eq!(registry.all()[1].id, "p1");
}

#[test]
fn test_registry_from_json_invalid_returns_error() {
    let json = r"not valid json";
    let result = PolicyRegistry::from_json(json);
    assert!(result.is_err(), "invalid JSON should return an error");
}

#[test]
fn test_registry_from_json_empty_array() {
    let json = "[]";
    let registry = PolicyRegistry::from_json(json).expect("empty array should parse");
    assert_eq!(
        registry.all().len(),
        0,
        "empty JSON array should produce empty registry"
    );
}

#[test]
fn test_registry_to_json_roundtrip() {
    let mut registry = PolicyRegistry::new();
    registry.add(
        RoutingPolicy::new("pol-1", "Test Policy")
            .with_priority(10)
            .with_capability(CapabilityCategory::Vision, "require"),
    );

    let json = registry.to_json().expect("serialization should succeed");
    // to_json serializes the Vec<RoutingPolicy> — check the id is present in the output
    assert!(
        json.contains("pol-1"),
        "serialized JSON should contain the policy id: {json}"
    );
    assert!(
        json.contains("Test Policy"),
        "serialized JSON should contain the policy name: {json}"
    );

    // Verify roundtrip: deserialize back and compare
    let deserialized: Vec<RoutingPolicy> =
        serde_json::from_str(&json).expect("roundtrip deserialization should succeed");
    assert_eq!(
        deserialized.len(),
        1,
        "roundtrip should preserve policy count"
    );
    assert_eq!(deserialized[0].id, "pol-1");
}

#[test]
fn test_registry_to_json_empty() {
    let registry = PolicyRegistry::new();
    let json = registry.to_json().expect("empty registry should serialize");
    assert_eq!(json, "[]");
}

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
    let mut policy_with_condition = policy;
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
    let policies_path = manifest_dir.join("config").join("policies.json");

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
