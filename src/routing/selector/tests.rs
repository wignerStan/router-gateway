use super::*;

#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use super::*;

    fn create_test_model() -> crate::registry::ModelInfo {
        crate::registry::ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window: 4096,
            max_output_tokens: 1024,
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: crate::registry::ModelCapabilities {
                streaming: true,
                tools: true,
                vision: false,
                thinking: false,
            },
            rate_limits: crate::registry::RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: crate::registry::DataSource::Static,
        }
    }

    mod basic_selection {
        use super::*;

        #[tokio::test]
        async fn test_weighted_selection() {
            let config = SmartRoutingConfig::default();
            let selector = SmartSelector::new(config);

            let auths = vec![
                AuthInfo {
                    id: "auth1".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth2".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth3".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
            ];

            for auth in &auths {
                selector.metrics().initialize_auth(&auth.id).await;
            }

            let selected = selector.pick(auths).await;
            assert!(selected.is_some());
            let selected_id = selected.unwrap();
            assert!(["auth1", "auth2", "auth3"].contains(&selected_id.as_str()));
        }

        #[tokio::test]
        async fn test_unavailable_filtering() {
            let config = SmartRoutingConfig::default();
            let selector = SmartSelector::new(config);

            let auths = vec![
                AuthInfo {
                    id: "auth1".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth2".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: true,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth3".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
            ];

            for auth in &auths {
                selector.metrics().initialize_auth(&auth.id).await;
            }

            for _ in 0..10 {
                let selected = selector.pick(auths.clone()).await;
                assert!(selected.is_some());
                let selected_id = selected.unwrap();
                assert_ne!(selected_id, "auth2");
            }
        }
    }

    mod pick_with_policy {
        use super::*;

        #[tokio::test]
        async fn test_pick_with_policy_basic() {
            let config = SmartRoutingConfig::default();
            let selector = SmartSelector::new(config);

            let auths = vec![
                AuthInfo {
                    id: "auth1".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth2".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
            ];

            for auth in &auths {
                selector.metrics().initialize_auth(&auth.id).await;
            }

            let model = create_test_model();
            let context = crate::registry::PolicyContext::default();

            let selected = selector.pick_with_policy(auths, &model, &context).await;
            assert!(selected.is_some());
            let selected_id = selected.unwrap();
            assert!(["auth1", "auth2"].contains(&selected_id.as_str()));
        }

        #[tokio::test]
        async fn test_pick_with_policy_disabled_routing() {
            let config = SmartRoutingConfig {
                enabled: false,
                ..Default::default()
            };
            let selector = SmartSelector::new(config);

            let auths = vec![
                AuthInfo {
                    id: "auth1".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth2".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
            ];

            let model = create_test_model();
            let context = crate::registry::PolicyContext::default();

            let selected = selector.pick_with_policy(auths, &model, &context).await;
            assert_eq!(selected, Some("auth1".to_string()));
        }

        #[tokio::test]
        async fn test_pick_with_policy_empty_auths() {
            let config = SmartRoutingConfig::default();
            let selector = SmartSelector::new(config);

            let auths: Vec<AuthInfo> = vec![];

            let model = create_test_model();
            let context = crate::registry::PolicyContext::default();

            let selected = selector.pick_with_policy(auths, &model, &context).await;
            assert!(selected.is_none());
        }

        #[tokio::test]
        async fn test_pick_with_policy_with_policy_registry() {
            use crate::registry::PolicyRegistry;

            let mut config = SmartRoutingConfig::default();
            config.policy.enabled = true;

            let registry = PolicyRegistry::new();
            let selector = SmartSelector::with_policy(config, registry);

            let auths = vec![AuthInfo {
                id: "auth1".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            }];

            selector.metrics().initialize_auth("auth1").await;

            let model = create_test_model();
            let context = crate::registry::PolicyContext::default();

            let selected = selector.pick_with_policy(auths, &model, &context).await;
            assert_eq!(selected, Some("auth1".to_string()));
        }
    }

    mod config_management {
        use super::*;

        #[tokio::test]
        async fn test_set_config() {
            let config = SmartRoutingConfig::default();
            let mut selector = SmartSelector::new(config);

            assert_eq!(selector.config().strategy, "weighted");

            let new_config = SmartRoutingConfig {
                strategy: "adaptive".to_string(),
                weight: crate::routing::config::WeightConfig {
                    success_rate_weight: 0.5,
                    ..Default::default()
                },
                ..Default::default()
            };

            selector.set_config(new_config);

            assert_eq!(selector.config().strategy, "adaptive");
            assert!((selector.config().weight.success_rate_weight - 0.5).abs() < 0.01);
        }

        #[tokio::test]
        async fn test_set_config_preserves_metrics() {
            let config = SmartRoutingConfig::default();
            let mut selector = SmartSelector::new(config);

            selector.metrics().initialize_auth("auth1").await;
            selector
                .metrics()
                .record_result("auth1", true, 100.0, 200)
                .await;

            let new_config = SmartRoutingConfig::default();
            selector.set_config(new_config);

            let metrics = selector.metrics().get_metrics("auth1").await;
            assert!(metrics.is_some());
            assert_eq!(metrics.unwrap().total_requests, 1);
        }

        #[tokio::test]
        async fn test_set_config_updates_health_config() {
            let config = SmartRoutingConfig::default();
            let mut selector = SmartSelector::new(config);

            let mut new_config = SmartRoutingConfig::default();
            new_config.health.healthy_threshold = 10;
            new_config.health.unhealthy_threshold = 20;

            selector.set_config(new_config);
        }
    }

    mod filter_and_weigh {
        use super::*;

        #[tokio::test]
        async fn test_filter_and_weigh_zero_weight_excluded() {
            let mut config = SmartRoutingConfig::default();
            config.health.unhealthy_threshold = 1;
            config.health.cooldown_period_seconds = 3600;
            let selector = SmartSelector::new(config);

            let auths = vec![
                AuthInfo {
                    id: "auth1".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth2".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
            ];

            for auth in &auths {
                selector.metrics().initialize_auth(&auth.id).await;
            }

            selector
                .health()
                .update_from_result("auth1", false, 500)
                .await;

            for _ in 0..10 {
                let selected = selector.pick(auths.clone()).await;
                assert!(selected.is_some());
            }
        }

        #[tokio::test]
        async fn test_filter_and_weigh_all_unavailable() {
            let config = SmartRoutingConfig::default();
            let selector = SmartSelector::new(config);

            let auths = vec![
                AuthInfo {
                    id: "auth1".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: true,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth2".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: true,
                    model_states: Vec::new(),
                },
            ];

            let selected = selector.pick(auths).await;
            assert!(selected.is_none());
        }

        #[tokio::test]
        async fn test_filter_and_weigh_single_available() {
            let config = SmartRoutingConfig::default();
            let selector = SmartSelector::new(config);

            let auths = vec![
                AuthInfo {
                    id: "auth1".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: true,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth2".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth3".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: true,
                    model_states: Vec::new(),
                },
            ];

            selector.metrics().initialize_auth("auth2").await;

            for _ in 0..5 {
                let selected = selector.pick(auths.clone()).await;
                assert_eq!(selected, Some("auth2".to_string()));
            }
        }
    }

    mod clone_behavior {
        use super::*;

        #[tokio::test]
        async fn test_selector_clone_shares_metrics_and_health() {
            let config = SmartRoutingConfig::default();
            let selector1 = SmartSelector::new(config);

            selector1.metrics().initialize_auth("auth1").await;
            selector1
                .metrics()
                .record_result("auth1", true, 100.0, 200)
                .await;

            let selector2 = selector1.clone();

            selector2.metrics().initialize_auth("auth2").await;
            selector2
                .metrics()
                .record_result("auth2", true, 50.0, 200)
                .await;

            assert!(selector1.metrics().get_metrics("auth2").await.is_some());
            assert!(selector2.metrics().get_metrics("auth1").await.is_some());
        }
    }

    mod selection_edge_cases {
        use super::*;

        #[tokio::test]
        async fn test_single_auth_selection() {
            let config = SmartRoutingConfig::default();
            let selector = SmartSelector::new(config);

            let auths = vec![AuthInfo {
                id: "only-auth".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            }];

            selector.metrics().initialize_auth("only-auth").await;

            for _ in 0..5 {
                let selected = selector.pick(auths.clone()).await;
                assert_eq!(selected, Some("only-auth".to_string()));
            }
        }

        #[tokio::test]
        async fn test_disabled_routing_returns_first() {
            let config = SmartRoutingConfig {
                enabled: false,
                ..Default::default()
            };
            let selector = SmartSelector::new(config);

            let auths = vec![
                AuthInfo {
                    id: "first-auth".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "second-auth".to_string(),
                    priority: Some(100),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
            ];

            let selected = selector.pick(auths).await;
            assert_eq!(selected, Some("first-auth".to_string()));
        }

        #[tokio::test]
        async fn test_all_zero_weights_falls_back_to_random() {
            let config = SmartRoutingConfig {
                weight: crate::routing::config::WeightConfig {
                    success_rate_weight: 0.0,
                    latency_weight: 0.0,
                    health_weight: 0.0,
                    load_weight: 0.0,
                    priority_weight: 0.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let selector = SmartSelector::new(config);

            let auths = vec![
                AuthInfo {
                    id: "auth1".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth2".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
            ];

            for auth in &auths {
                selector.metrics().initialize_auth(&auth.id).await;
            }

            let mut selections = std::collections::HashSet::new();
            for _ in 0..50 {
                if let Some(id) = selector.pick(auths.clone()).await {
                    selections.insert(id);
                }
            }

            assert!(
                !selections.is_empty(),
                "Should be able to select with zero weights"
            );
        }

        #[tokio::test]
        async fn test_quota_exceeded_massively_reduces_selection_probability() {
            let config = SmartRoutingConfig::default();
            let selector = SmartSelector::new(config);

            let auths = vec![
                AuthInfo {
                    id: "normal-auth".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "quota-auth".to_string(),
                    priority: Some(0),
                    quota_exceeded: true,
                    unavailable: false,
                    model_states: Vec::new(),
                },
            ];

            for auth in &auths {
                selector.metrics().initialize_auth(&auth.id).await;
            }

            let mut normal_count = 0;
            let mut quota_count = 0;
            for _ in 0..500 {
                let selected = selector.pick(auths.clone()).await.unwrap();
                if selected == "normal-auth" {
                    normal_count += 1;
                } else {
                    quota_count += 1;
                }
            }

            assert!(
                normal_count > quota_count * 4,
                "Normal auth should dominate over quota-exceeded (normal: {normal_count}, quota: {quota_count})"
            );
        }
    }

    mod floating_point_precision {
        use super::*;

        #[tokio::test]
        async fn test_weighted_selection_with_very_small_differences() {
            let config = SmartRoutingConfig::default();
            let selector = SmartSelector::new(config);

            let auths = vec![
                AuthInfo {
                    id: "auth1".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth2".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
            ];

            selector.metrics().initialize_auth("auth1").await;
            selector.metrics().initialize_auth("auth2").await;

            selector
                .metrics()
                .record_result("auth1", true, 100.0, 200)
                .await;
            selector
                .metrics()
                .record_result("auth2", true, 100.0001, 200)
                .await;

            let mut auth1_count = 0;
            let mut auth2_count = 0;
            for _ in 0..100 {
                let selected = selector.pick(auths.clone()).await.unwrap();
                if selected == "auth1" {
                    auth1_count += 1;
                } else {
                    auth2_count += 1;
                }
            }

            assert!(
                auth1_count > 20 && auth2_count > 20,
                "Both auths should receive selections with near-equal weights (auth1: {auth1_count}, auth2: {auth2_count})"
            );
        }

        #[tokio::test]
        async fn test_weighted_selection_extreme_weight_ratio() {
            let config = SmartRoutingConfig::default();
            let selector = SmartSelector::new(config);

            let auths = vec![
                AuthInfo {
                    id: "high-perf".to_string(),
                    priority: Some(100),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "low-perf".to_string(),
                    priority: Some(-100),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
            ];

            selector.metrics().initialize_auth("high-perf").await;
            selector.metrics().initialize_auth("low-perf").await;

            for _ in 0..50 {
                selector
                    .metrics()
                    .record_result("high-perf", true, 1.0, 200)
                    .await;
                selector
                    .metrics()
                    .record_result("low-perf", false, 30000.0, 500)
                    .await;
            }

            let mut high_count = 0;
            let mut low_count = 0;
            for _ in 0..100 {
                match selector.pick(auths.clone()).await.unwrap().as_str() {
                    "high-perf" => high_count += 1,
                    "low-perf" => low_count += 1,
                    _ => {},
                }
            }

            assert!(
                high_count > low_count,
                "High-perf auth should be selected more often than low-perf (high: {high_count}, low: {low_count})"
            );

            assert!(
                high_count >= 60,
                "High-perf auth should be selected at least 60% of the time (got {high_count} out of 100)"
            );
        }
    }

    mod concurrent_access {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
        async fn test_concurrent_pick_operations() {
            use std::sync::Arc;

            let config = SmartRoutingConfig::default();
            let selector = Arc::new(SmartSelector::new(config));

            let auths = vec![
                AuthInfo {
                    id: "auth1".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth2".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth3".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
            ];

            for auth in &auths {
                selector.metrics().initialize_auth(&auth.id).await;
            }

            let mut handles = Vec::new();
            for _ in 0..20 {
                let selector_clone = Arc::clone(&selector);
                let auths_clone = auths.clone();
                let handle = tokio::spawn(async move {
                    let mut results = Vec::new();
                    for _ in 0..10 {
                        let selected = selector_clone.pick(auths_clone.clone()).await;
                        results.push(selected);
                    }
                    results
                });
                handles.push(handle);
            }

            let all_results: Vec<_> = futures::future::join_all(handles).await;

            for results in all_results {
                for selected in results.unwrap() {
                    assert!(selected.is_some());
                    let id = selected.unwrap();
                    assert!(
                        ["auth1", "auth2", "auth3"].contains(&id.as_str()),
                        "Selected invalid auth: {id}"
                    );
                }
            }
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
        async fn test_concurrent_pick_and_record() {
            use std::sync::Arc;

            let config = SmartRoutingConfig::default();
            let selector = Arc::new(SmartSelector::new(config));

            let auths = vec![
                AuthInfo {
                    id: "auth1".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "auth2".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
            ];

            for auth in &auths {
                selector.metrics().initialize_auth(&auth.id).await;
            }

            let mut handles = Vec::new();

            for _ in 0..10 {
                let selector_clone = Arc::clone(&selector);
                let auths_clone = auths.clone();
                let handle = tokio::spawn(async move {
                    for _ in 0..20 {
                        let _ = selector_clone.pick(auths_clone.clone()).await;
                    }
                });
                handles.push(handle);
            }

            for i in 0..10 {
                let selector_clone = Arc::clone(&selector);
                let auth_id = format!("auth{}", (i % 2) + 1);
                let handle = tokio::spawn(async move {
                    for j in 0..20 {
                        selector_clone
                            .metrics()
                            .record_result(&auth_id, j % 2 == 0, 100.0 + f64::from(j), 200)
                            .await;
                    }
                });
                handles.push(handle);
            }

            let results: Vec<_> = futures::future::join_all(handles).await;

            for result in results {
                assert!(result.is_ok(), "Task panicked during concurrent operations");
            }
        }
    }

    mod boundary_conditions {
        use super::*;

        #[tokio::test]
        async fn test_pick_with_large_number_of_auths() {
            let config = SmartRoutingConfig::default();
            let selector = SmartSelector::new(config);

            let auths: Vec<AuthInfo> = (0..100)
                .map(|i| AuthInfo {
                    id: format!("auth-{i}"),
                    priority: Some(i % 10 - 5),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                })
                .collect();

            for auth in &auths {
                selector.metrics().initialize_auth(&auth.id).await;
            }

            let selected = selector.pick(auths.clone()).await;
            assert!(selected.is_some());

            let mut all_selected = std::collections::HashSet::new();
            for _ in 0..500 {
                if let Some(id) = selector.pick(auths.clone()).await {
                    all_selected.insert(id);
                }
            }

            assert!(
                all_selected.len() > 10,
                "Should select multiple different auths from large pool"
            );
        }

        #[tokio::test]
        async fn test_pick_with_duplicate_auth_ids() {
            let config = SmartRoutingConfig::default();
            let selector = SmartSelector::new(config);

            let auths = vec![
                AuthInfo {
                    id: "same-auth".to_string(),
                    priority: Some(0),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
                AuthInfo {
                    id: "same-auth".to_string(),
                    priority: Some(100),
                    quota_exceeded: false,
                    unavailable: false,
                    model_states: Vec::new(),
                },
            ];

            selector.metrics().initialize_auth("same-auth").await;

            let selected = selector.pick(auths).await;
            assert_eq!(selected, Some("same-auth".to_string()));
        }
    }
}
