use chrono::{DateTime, Utc};
use smart_routing::config::WeightConfig;
use smart_routing::{
    ExecutionOutcome, HealthConfig, HealthManager, HealthStatus, RouteStatistics,
    SmartRoutingConfig, TimeBucket,
};

// ============================================================
// SmartRoutingConfig::validate() error reporting
// ============================================================

#[test]
fn validate_warns_on_out_of_range_weights() {
    let mut config = SmartRoutingConfig {
        weight: WeightConfig {
            success_rate_weight: 5.0,
            latency_weight: -1.0,
            health_weight: 2.0,
            load_weight: 0.5,
            priority_weight: 0.5,
            ..Default::default()
        },
        ..Default::default()
    };
    let warnings = config.validate().unwrap();
    assert!(
        !warnings.is_empty(),
        "out-of-range weights should produce warnings"
    );
    assert!(
        warnings.iter().any(|w| w.contains("success_rate_weight")),
        "warnings should mention success_rate_weight: {warnings:?}"
    );
    assert!(
        warnings.iter().any(|w| w.contains("latency_weight")),
        "warnings should mention latency_weight: {warnings:?}"
    );
    assert!(
        warnings.iter().any(|w| w.contains("health_weight")),
        "warnings should mention health_weight: {warnings:?}"
    );
}

#[test]
fn validate_warns_on_invalid_strategy() {
    let mut config = SmartRoutingConfig {
        strategy: "bogus_strategy".to_string(),
        ..Default::default()
    };
    let warnings = config.validate().unwrap();
    assert!(
        warnings.iter().any(|w| w.contains("bogus_strategy")),
        "warnings should mention the invalid strategy: {warnings:?}"
    );
    assert_eq!(
        config.strategy, "weighted",
        "invalid strategy should be reset to weighted"
    );
}

#[test]
fn validate_warns_on_negative_values() {
    let mut config = SmartRoutingConfig {
        time_aware: smart_routing::TimeAwareConfig {
            off_peak_factor: -0.5,
            ..Default::default()
        },
        ..Default::default()
    };
    let warnings = config.validate().unwrap();
    assert!(
        warnings.iter().any(|w| w.contains("off_peak_factor")),
        "warnings should mention off_peak_factor: {warnings:?}"
    );
    assert!(
        (config.time_aware.off_peak_factor - 1.2).abs() < 0.01,
        "negative off_peak_factor should be reset to 1.2"
    );
}

#[test]
fn validate_no_warnings_for_valid_config() {
    let mut config = SmartRoutingConfig::default();
    let warnings = config.validate().unwrap();
    assert!(
        warnings.is_empty(),
        "valid default config should produce no warnings, got: {warnings:?}"
    );
}

// ============================================================
// All 5 strategies accepted by validate()
// ============================================================

#[test]
fn validate_accepts_all_five_strategies() {
    let strategies = [
        "weighted",
        "time_aware",
        "quota_aware",
        "adaptive",
        "policy_aware",
    ];
    for strategy in &strategies {
        let mut config = SmartRoutingConfig {
            strategy: strategy.to_string(),
            ..Default::default()
        };
        let warnings = config.validate().unwrap();
        assert!(
            warnings.iter().all(|w| !w.contains("strategy")),
            "strategy '{strategy}' should not produce strategy warnings, got: {warnings:?}"
        );
        assert_eq!(
            config.strategy, *strategy,
            "strategy '{strategy}' should be preserved"
        );
    }
}

// ============================================================
// Strategy alignment: smart-routing and gateway share the same set
// ============================================================

#[test]
fn strategy_set_matches_gateway_valid_strategies() {
    // Both gateway (apps/gateway/src/config.rs) and smart-routing accept exactly these:
    let aligned = [
        "weighted",
        "time_aware",
        "quota_aware",
        "adaptive",
        "policy_aware",
    ];
    let mut config = SmartRoutingConfig::default();
    for strategy in &aligned {
        config.strategy = strategy.to_string();
        config.validate().unwrap();
        assert_eq!(config.strategy, *strategy);
    }
}

// ============================================================
// PolicyRegistry::from_file() JSON schema validation
// ============================================================

#[test]
fn policy_registry_loads_valid_policies_file() {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set");
    let path = format!("{manifest_dir}/../../config/policies.json");
    let registry = model_registry::PolicyRegistry::from_file(&path);
    assert!(
        registry.is_ok(),
        "policies.json should load successfully: {:?}",
        registry.err()
    );
    let policies = registry.unwrap();
    assert!(policies.all().len() >= 5, "should load multiple policies");
}

#[test]
fn policy_registry_rejects_invalid_json() {
    let dir = std::env::temp_dir().join("smart_routing_test_invalid_policy");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("broken.json");
    std::fs::write(&path, r#"{"policies": [{"bad": true}]}"#).unwrap();
    // The JSON is structurally valid but should fail schema validation
    // because it doesn't have required fields like "id"
    let result = model_registry::PolicyRegistry::from_file(&path);
    assert!(
        result.is_err(),
        "invalid policy JSON should fail schema validation"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

// ============================================================
// Weekend/weekday statistics isolation (TimeBucket)
// ============================================================

#[test]
fn weekday_peak_isolated_from_weekend_peak() {
    // Monday 10:00 -> WeekdayPeak, not WeekendPeak
    let ts = DateTime::parse_from_rfc3339("2026-03-09T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let buckets = TimeBucket::from_timestamp(ts);
    assert!(
        buckets.contains(&TimeBucket::WeekdayPeak),
        "Monday 10am should be WeekdayPeak"
    );
    assert!(
        !buckets.contains(&TimeBucket::WeekendPeak),
        "Monday 10am should NOT be WeekendPeak"
    );
    assert!(
        !buckets.contains(&TimeBucket::WeekendOffPeak),
        "Monday 10am should NOT be WeekendOffPeak"
    );
}

#[test]
fn weekend_peak_isolated_from_weekday_peak() {
    // Saturday 14:00 -> WeekendPeak, not WeekdayPeak
    let ts = DateTime::parse_from_rfc3339("2026-03-14T14:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let buckets = TimeBucket::from_timestamp(ts);
    assert!(
        buckets.contains(&TimeBucket::WeekendPeak),
        "Saturday 2pm should be WeekendPeak"
    );
    assert!(
        !buckets.contains(&TimeBucket::WeekdayPeak),
        "Saturday 2pm should NOT be WeekdayPeak"
    );
    assert!(
        !buckets.contains(&TimeBucket::WeekdayOffPeak),
        "Saturday 2pm should NOT be WeekdayOffPeak"
    );
}

#[test]
fn weekday_offpeak_isolated_from_all_others() {
    // Tuesday 03:00 -> WeekdayOffPeak only
    let ts = DateTime::parse_from_rfc3339("2026-03-10T03:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let buckets = TimeBucket::from_timestamp(ts);
    assert!(buckets.contains(&TimeBucket::WeekdayOffPeak));
    assert!(!buckets.contains(&TimeBucket::WeekdayPeak));
    assert!(!buckets.contains(&TimeBucket::WeekendPeak));
    assert!(!buckets.contains(&TimeBucket::WeekendOffPeak));
}

#[test]
fn weekend_offpeak_isolated_from_all_others() {
    // Sunday 02:00 -> WeekendOffPeak only
    let ts = DateTime::parse_from_rfc3339("2026-03-15T02:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let buckets = TimeBucket::from_timestamp(ts);
    assert!(buckets.contains(&TimeBucket::WeekendOffPeak));
    assert!(!buckets.contains(&TimeBucket::WeekdayOffPeak));
    assert!(!buckets.contains(&TimeBucket::WeekdayPeak));
    assert!(!buckets.contains(&TimeBucket::WeekendPeak));
}

#[test]
fn compound_buckets_record_to_independent_stats() {
    let mut stats = RouteStatistics::new("route-1".to_string());

    // Weekday peak event (Monday 10am)
    let ts_wp = DateTime::parse_from_rfc3339("2026-03-09T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let mut outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 200, 300, 200);
    outcome.timestamp = ts_wp;
    stats.update(&outcome);

    // Weekend off-peak event (Sunday 2am)
    let ts_wo = DateTime::parse_from_rfc3339("2026-03-15T02:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let mut outcome2 = ExecutionOutcome::success("route-1".to_string(), 50.0, 100, 150, 200);
    outcome2.timestamp = ts_wo;
    stats.update(&outcome2);

    assert_eq!(
        stats
            .get_bucket_stats(&TimeBucket::WeekdayPeak)
            .unwrap()
            .total_requests,
        1,
        "WeekdayPeak should have exactly 1 request"
    );
    assert_eq!(
        stats
            .get_bucket_stats(&TimeBucket::WeekendOffPeak)
            .unwrap()
            .total_requests,
        1,
        "WeekendOffPeak should have exactly 1 request"
    );
    assert_eq!(
        stats
            .get_bucket_stats(&TimeBucket::WeekendPeak)
            .map(|s| s.total_requests)
            .unwrap_or(0),
        0,
        "WeekendPeak should have 0 requests"
    );
    assert_eq!(
        stats
            .get_bucket_stats(&TimeBucket::WeekdayOffPeak)
            .map(|s| s.total_requests)
            .unwrap_or(0),
        0,
        "WeekdayOffPeak should have 0 requests"
    );
}

// ============================================================
// Boundary hours for time buckets
// ============================================================

#[test]
fn peak_starts_at_hour_9() {
    let ts = DateTime::parse_from_rfc3339("2026-03-09T09:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let buckets = TimeBucket::from_timestamp(ts);
    assert!(buckets.contains(&TimeBucket::Peak), "9am should be Peak");
    assert!(
        buckets.contains(&TimeBucket::WeekdayPeak),
        "Monday 9am should be WeekdayPeak"
    );
}

#[test]
fn offpeak_starts_at_hour_21() {
    let ts = DateTime::parse_from_rfc3339("2026-03-09T21:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let buckets = TimeBucket::from_timestamp(ts);
    assert!(
        buckets.contains(&TimeBucket::OffPeak),
        "9pm should be OffPeak"
    );
    assert!(
        buckets.contains(&TimeBucket::WeekdayOffPeak),
        "Monday 9pm should be WeekdayOffPeak"
    );
}

#[test]
fn hour_8_is_offpeak() {
    let ts = DateTime::parse_from_rfc3339("2026-03-09T08:59:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let buckets = TimeBucket::from_timestamp(ts);
    assert!(
        buckets.contains(&TimeBucket::OffPeak),
        "8:59am should be OffPeak"
    );
    assert!(
        buckets.contains(&TimeBucket::WeekdayOffPeak),
        "Monday 8:59am should be WeekdayOffPeak"
    );
}

#[test]
fn hour_20_is_still_peak() {
    let ts = DateTime::parse_from_rfc3339("2026-03-09T20:59:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let buckets = TimeBucket::from_timestamp(ts);
    assert!(buckets.contains(&TimeBucket::Peak), "8:59pm should be Peak");
    assert!(
        buckets.contains(&TimeBucket::WeekdayPeak),
        "Monday 8:59pm should be WeekdayPeak"
    );
}

// ============================================================
// HealthManager clone independence
// ============================================================

#[tokio::test]
async fn health_manager_clone_starts_empty() {
    let config = HealthConfig::default();
    let manager = HealthManager::new(config);

    // Add health events to original
    manager.update_from_result("auth-1", false, 500).await;
    manager.update_from_result("auth-1", false, 500).await;
    manager.update_from_result("auth-1", false, 500).await;
    assert_eq!(
        manager.get_status("auth-1").await,
        HealthStatus::Unhealthy,
        "original should see unhealthy auth-1"
    );

    // Clone should start empty
    let clone = manager.clone();
    assert_eq!(
        clone.get_status("auth-1").await,
        HealthStatus::Healthy,
        "clone should start with empty health storage, defaulting to Healthy"
    );
    assert!(
        clone.get_health("auth-1").await.is_none(),
        "clone should have no health record for auth-1"
    );
}

#[tokio::test]
async fn health_manager_clone_updates_are_independent() {
    let config = HealthConfig::default();
    let manager1 = HealthManager::new(config);

    manager1.update_from_result("auth-1", true, 200).await;

    let manager2 = manager1.clone();

    // Update clone
    manager2.update_from_result("auth-2", false, 500).await;
    manager2.update_from_result("auth-2", false, 500).await;
    manager2.update_from_result("auth-2", false, 500).await;

    // Original should not see auth-2
    assert!(
        manager1.get_health("auth-2").await.is_none(),
        "original should not see updates made to the clone"
    );
    assert_eq!(
        manager1.get_status("auth-2").await,
        HealthStatus::Healthy,
        "original should see auth-2 as Healthy (no record exists)"
    );

    // Clone should see auth-2 as unhealthy
    assert_eq!(
        manager2.get_status("auth-2").await,
        HealthStatus::Unhealthy,
        "clone should see auth-2 as unhealthy"
    );

    // Update original — clone should not be affected
    manager1.update_from_result("auth-3", false, 500).await;
    manager1.update_from_result("auth-3", false, 500).await;
    manager1.update_from_result("auth-3", false, 500).await;
    assert!(
        manager2.get_health("auth-3").await.is_none(),
        "clone should not see updates made to the original"
    );
}

#[tokio::test]
async fn health_manager_multiple_clones_are_independent() {
    let config = HealthConfig::default();
    let original = HealthManager::new(config);
    original.update_from_result("shared", true, 200).await;

    let clone_a = original.clone();
    let clone_b = original.clone();

    clone_a.update_from_result("a-only", false, 500).await;
    clone_a.update_from_result("a-only", false, 500).await;
    clone_a.update_from_result("a-only", false, 500).await;

    clone_b.update_from_result("b-only", true, 200).await;

    // Each clone sees its own updates only
    assert_eq!(clone_a.get_status("a-only").await, HealthStatus::Unhealthy);
    assert_eq!(clone_b.get_status("a-only").await, HealthStatus::Healthy);
    assert_eq!(clone_a.get_status("b-only").await, HealthStatus::Healthy);
    assert!(clone_b.get_health("b-only").await.is_some());

    // Neither clone sees the original's "shared" event
    assert!(clone_a.get_health("shared").await.is_none());
    assert!(clone_b.get_health("shared").await.is_none());
}

// ============================================================
// validate() returns specific warning messages
// ============================================================

#[test]
fn validate_warnings_contain_field_names() {
    let mut config = SmartRoutingConfig {
        strategy: "unknown".to_string(),
        weight: WeightConfig {
            success_rate_weight: 10.0,
            latency_weight: -5.0,
            health_weight: 1.5,
            load_weight: -0.1,
            priority_weight: 2.0,
            ..Default::default()
        },
        quota_aware: smart_routing::QuotaAwareConfig {
            quota_balance_strategy: "bad_strategy".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    let warnings = config.validate().unwrap();

    // Strategy warning
    assert!(
        warnings.iter().any(|w| w.contains("unknown")),
        "should warn about invalid strategy"
    );
    // Weight warnings
    assert!(
        warnings.iter().any(|w| w.contains("success_rate_weight")),
        "should warn about success_rate_weight"
    );
    assert!(
        warnings.iter().any(|w| w.contains("latency_weight")),
        "should warn about latency_weight"
    );
    assert!(
        warnings.iter().any(|w| w.contains("priority_weight")),
        "should warn about priority_weight"
    );
    // Quota strategy warning
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("quota_balance_strategy")),
        "should warn about quota_balance_strategy"
    );
}
