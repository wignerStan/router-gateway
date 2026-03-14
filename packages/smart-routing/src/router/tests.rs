use super::*;
use crate::classification::{
    ClassifiedRequest, QualityPreference, RequestFormat, RequiredCapabilities,
};
use model_registry::{DataSource, ModelCapabilities, ModelInfo, RateLimits};

fn create_test_model(id: &str, provider: &str, context_window: usize) -> ModelInfo {
    ModelInfo {
        id: id.to_string(),
        name: format!("Test Model {id}"),
        provider: provider.to_string(),
        context_window,
        max_output_tokens: 4096,
        input_price_per_million: 1.0,
        output_price_per_million: 2.0,
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

fn create_test_request(estimated_tokens: u32) -> ClassifiedRequest {
    ClassifiedRequest {
        required_capabilities: RequiredCapabilities::default(),
        estimated_tokens,
        format: RequestFormat::OpenAI,
        quality_preference: QualityPreference::Balanced,
    }
}

fn create_test_auth(id: &str) -> crate::weight::AuthInfo {
    crate::weight::AuthInfo {
        id: id.to_string(),
        priority: Some(0),
        quota_exceeded: false,
        unavailable: false,
        model_states: Vec::new(),
    }
}

mod creation_and_defaults {
    use super::*;

    #[tokio::test]
    async fn test_router_creation() {
        let router = Router::new();
        let _router2 = router;
    }

    #[tokio::test]
    async fn test_router_default() {
        let _router = Router::default();
        let _router2 = Router::new();
        // Both should be equivalent
    }

    #[tokio::test]
    async fn test_router_config_defaults() {
        let config = RouterConfig::default();
        assert!(config.use_bandit);
        assert_eq!(config.max_fallbacks, 5);
        assert_eq!(config.min_fallbacks, 2);
        assert!(config.enable_provider_diversity);
        assert!(config.enable_session_affinity);
    }

    #[tokio::test]
    async fn test_router_with_custom_config() {
        let config = RouterConfig {
            use_bandit: false,
            max_fallbacks: 3,
            min_fallbacks: 1,
            enable_provider_diversity: false,
            enable_session_affinity: false,
        };

        let router = Router::with_config(config);
        assert!(!router.config().use_bandit);
        assert_eq!(router.config().max_fallbacks, 3);
    }

    #[tokio::test]
    async fn test_router_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Router>();
    }
}

mod credential_management {
    use super::*;

    #[tokio::test]
    async fn test_router_add_credential() {
        let mut router = Router::new();
        router.add_credential("cred-1".to_string(), vec!["laude-3-opus".to_string()]);
        router.set_model(
            "laude-3-opus".to_string(),
            create_test_model("laude-3-opus", "anthropic", 200000),
        );
    }

    #[tokio::test]
    async fn test_router_clone_preserves_credentials() {
        let mut router1 = Router::new();
        router1.add_credential("cred-1".to_string(), vec!["model-a".to_string()]);
        router1.set_model(
            "model-a".to_string(),
            create_test_model("model-a", "provider-a", 200000),
        );
        router1.add_disabled_provider("blocked".to_string());

        let router2 = router1.clone();

        // Clone should preserve credentials - no need to re-register
        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1")];

        router2.metrics().initialize_auth("cred-1").await;
        let plan = router2.plan(&request, auths, None).await;

        assert!(
            plan.primary.is_some(),
            "Clone should preserve registered credentials"
        );
    }

    #[tokio::test]
    async fn test_router_clone_preserves_bandit_state() {
        let router1 = Router::new();
        router1.metrics().initialize_auth("cred-1").await;
        router1.record_result("cred-1", true, 100.0, 200, 0.8).await;

        let router2 = router1.clone();

        // Bandit state is shared via Arc - both see the same recorded result
        let stats = router2
            .bandit_policy()
            .lock()
            .await
            .get_stats("cred-1")
            .cloned();
        assert!(stats.is_some(), "Clone should preserve bandit policy state");
        let s = stats.expect("Route statistics should be available");
        assert_eq!(s.pulls, 1);
        assert_eq!(s.last_utility, 0.8);
    }

    #[tokio::test]
    async fn test_router_record_result() {
        let router = Router::new();

        // Record a result
        router.record_result("cred-1", true, 100.0, 200, 0.8).await;

        // Verify bandit policy recorded it
        let stats = router
            .bandit_policy()
            .lock()
            .await
            .get_stats("cred-1")
            .cloned();
        assert!(stats.is_some());
    }

    #[tokio::test]
    async fn test_router_disabled_provider() {
        let mut router = Router::new();
        router.add_disabled_provider("blocked-provider".to_string());
        router.add_credential("cred-1".to_string(), vec!["model-1".to_string()]);
        router.set_model(
            "model-1".to_string(),
            create_test_model("model-1", "blocked-provider", 200000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1")];

        let plan = router.plan(&request, auths, None).await;

        // Should be filtered due to disabled provider
        assert!(plan.primary.is_none());
    }

    #[tokio::test]
    async fn test_router_multiple_credentials() {
        let mut router = Router::new();
        router.add_credential("cred-1".to_string(), vec!["laude-3-opus".to_string()]);
        router.add_credential("cred-2".to_string(), vec!["gpt-4".to_string()]);
        router.add_credential("cred-3".to_string(), vec!["gemini-pro".to_string()]);
        router.set_model(
            "laude-3-opus".to_string(),
            create_test_model("laude-3-opus", "anthropic", 200000),
        );
        router.set_model(
            "gpt-4".to_string(),
            create_test_model("gpt-4", "openai", 128000),
        );
        router.set_model(
            "gemini-pro".to_string(),
            create_test_model("gemini-pro", "google", 128000),
        );

        let request = create_test_request(1000);
        let auths = vec![
            create_test_auth("cred-1"),
            create_test_auth("cred-2"),
            create_test_auth("cred-3"),
        ];

        for auth in &auths {
            router.metrics().initialize_auth(&auth.id).await;
        }

        let plan = router.plan(&request, auths, None).await;

        assert!(plan.primary.is_some());
        assert_eq!(plan.total_candidates, 3);
        assert!(plan.filtered_candidates >= 1);
    }
}

mod plan_operations {
    use super::*;

    #[tokio::test]
    async fn test_router_plan_with_valid_candidates() {
        let mut router = Router::new();
        router.add_credential("cred-1".to_string(), vec!["laude-3-opus".to_string()]);
        router.set_model(
            "laude-3-opus".to_string(),
            create_test_model("laude-3-opus", "anthropic", 200000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1")];

        // Initialize metrics
        router.metrics().initialize_auth("cred-1").await;

        let plan = router.plan(&request, auths, None).await;

        assert!(plan.primary.is_some());
        assert_eq!(plan.total_candidates, 1);
        assert_eq!(plan.filtered_candidates, 1);
    }

    #[tokio::test]
    async fn test_router_plan_with_no_credentials() {
        let router = Router::new();

        let request = create_test_request(1000);
        let auths: Vec<crate::weight::AuthInfo> = vec![];

        let plan = router.plan(&request, auths, None).await;

        assert!(plan.primary.is_none());
        assert_eq!(plan.total_candidates, 0);
        assert_eq!(plan.filtered_candidates, 0);
    }

    #[tokio::test]
    async fn test_router_plan_with_filtered_candidates() {
        let mut router = Router::new();
        router.add_credential("cred-1".to_string(), vec!["gpt-4".to_string()]);
        // Set a model with small context window
        router.set_model(
            "gpt-4".to_string(),
            create_test_model("gpt-4", "openai", 10000),
        );

        // Request exceeds context window
        let request = create_test_request(50000);
        let auths = vec![create_test_auth("cred-1")];

        let plan = router.plan(&request, auths, None).await;

        // Should be filtered due to context overflow
        assert!(plan.primary.is_none());
        assert_eq!(plan.total_candidates, 1);
        assert_eq!(plan.filtered_candidates, 0);
    }

    #[tokio::test]
    async fn test_route_plan_item_fields() {
        let mut router = Router::new();
        router.add_credential("cred-1".to_string(), vec!["laude-3-opus".to_string()]);
        router.set_model(
            "laude-3-opus".to_string(),
            create_test_model("laude-3-opus", "anthropic", 200000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1")];

        router.metrics().initialize_auth("cred-1").await;

        let plan = router.plan(&request, auths, None).await;

        assert!(plan.primary.is_some());
        let primary = plan
            .primary
            .expect("Execution plan should have a primary route");
        assert_eq!(primary.credential_id, "cred-1");
        assert_eq!(primary.model_id, "laude-3-opus");
        assert_eq!(primary.provider, "anthropic");
        assert!(primary.utility >= 0.0);
        assert!(primary.weight >= 0.0);
    }
}

mod fallback_generation {
    use super::*;

    #[tokio::test]
    async fn test_router_plan_generates_fallbacks() {
        let mut router = Router::new();
        router.add_credential("cred-1".to_string(), vec!["laude-3-opus".to_string()]);
        router.add_credential("cred-2".to_string(), vec!["gpt-4".to_string()]);
        router.set_model(
            "laude-3-opus".to_string(),
            create_test_model("laude-3-opus", "anthropic", 200000),
        );
        router.set_model(
            "gpt-4".to_string(),
            create_test_model("gpt-4", "openai", 128000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1"), create_test_auth("cred-2")];

        // Initialize metrics
        router.metrics().initialize_auth("cred-1").await;
        router.metrics().initialize_auth("cred-2").await;

        let plan = router.plan(&request, auths, None).await;

        assert!(plan.primary.is_some());
        assert!(!plan.fallbacks.is_empty());
    }

    #[tokio::test]
    async fn test_router_generates_correct_number_of_fallbacks() {
        let config = RouterConfig {
            max_fallbacks: 3,
            min_fallbacks: 2,
            ..Default::default()
        };
        let mut router = Router::with_config(config);
        router.add_credential("cred-1".to_string(), vec!["laude-3-opus".to_string()]);
        router.add_credential("cred-2".to_string(), vec!["gpt-4".to_string()]);
        router.add_credential("cred-3".to_string(), vec!["gemini-pro".to_string()]);
        router.add_credential("cred-4".to_string(), vec!["llama-3".to_string()]);
        router.set_model(
            "laude-3-opus".to_string(),
            create_test_model("laude-3-opus", "anthropic", 200000),
        );
        router.set_model(
            "gpt-4".to_string(),
            create_test_model("gpt-4", "openai", 128000),
        );
        router.set_model(
            "gemini-pro".to_string(),
            create_test_model("gemini-pro", "google", 128000),
        );
        router.set_model(
            "llama-3".to_string(),
            create_test_model("llama-3", "meta", 128000),
        );

        let request = create_test_request(1000);
        let auths = vec![
            create_test_auth("cred-1"),
            create_test_auth("cred-2"),
            create_test_auth("cred-3"),
            create_test_auth("cred-4"),
        ];

        for auth in &auths {
            router.metrics().initialize_auth(&auth.id).await;
        }

        let plan = router.plan(&request, auths, None).await;

        assert!(plan.primary.is_some());
        // Fallbacks should be <= max_fallbacks (3)
        assert!(
            plan.fallbacks.len() <= 3,
            "Fallbacks should not exceed max_fallbacks"
        );
    }

    #[tokio::test]
    async fn test_router_fallback_count_with_limited_candidates() {
        let config = RouterConfig {
            max_fallbacks: 10,
            min_fallbacks: 5, // More than available
            ..Default::default()
        };
        let mut router = Router::with_config(config);
        // Only 2 credentials available
        router.add_credential("cred-1".to_string(), vec!["laude-3-opus".to_string()]);
        router.add_credential("cred-2".to_string(), vec!["gpt-4".to_string()]);
        router.set_model(
            "laude-3-opus".to_string(),
            create_test_model("laude-3-opus", "anthropic", 200000),
        );
        router.set_model(
            "gpt-4".to_string(),
            create_test_model("gpt-4", "openai", 128000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1"), create_test_auth("cred-2")];

        for auth in &auths {
            router.metrics().initialize_auth(&auth.id).await;
        }

        let plan = router.plan(&request, auths, None).await;

        assert!(plan.primary.is_some());
        // Should return available fallbacks, not min_fallbacks
        assert!(
            plan.fallbacks.len() <= 2,
            "Should return available auths as fallbacks, not min_fallbacks"
        );
    }

    #[tokio::test]
    async fn test_router_with_zero_max_fallbacks() {
        let config = RouterConfig {
            max_fallbacks: 0,
            min_fallbacks: 0,
            ..Default::default()
        };
        let mut router = Router::with_config(config);
        router.add_credential("cred-1".to_string(), vec!["laude-3-opus".to_string()]);
        router.add_credential("cred-2".to_string(), vec!["gpt-4".to_string()]);
        router.set_model(
            "laude-3-opus".to_string(),
            create_test_model("laude-3-opus", "anthropic", 200000),
        );
        router.set_model(
            "gpt-4".to_string(),
            create_test_model("gpt-4", "openai", 128000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1"), create_test_auth("cred-2")];

        for auth in &auths {
            router.metrics().initialize_auth(&auth.id).await;
        }

        let plan = router.plan(&request, auths, None).await;

        assert!(plan.primary.is_some());
        assert!(
            plan.fallbacks.is_empty(),
            "Zero max_fallbacks should return no fallbacks"
        );
    }
}

mod edge_cases {
    use super::*;

    #[tokio::test]
    async fn test_router_handles_credential_model_mismatch_gracefully() {
        let mut router = Router::new();
        // Register credential with one model
        router.add_credential("cred-1".to_string(), vec!["laude-3-opus".to_string()]);
        // But don't register the model info - this simulates mismatch

        let request = create_test_request(1000);
        // Auth references non-existent model
        let auths = vec![create_test_auth("cred-1")];

        let plan = router.plan(&request, auths, None).await;

        // Should handle gracefully - either no route or empty plan
        // The candidate builder will create no candidates without model info
        assert_eq!(plan.total_candidates, 0);
    }

    #[tokio::test]
    async fn test_router_record_result_updates_all_subsystems() {
        let router = Router::new();

        // Initialize the credential in metrics first
        router.metrics().initialize_auth("cred-1").await;

        // Record a result
        router.record_result("cred-1", true, 150.0, 200, 0.9).await;

        // Verify metrics were updated
        let metrics = router.metrics().get_metrics("cred-1").await;
        assert!(metrics.is_some(), "Metrics should be recorded");
        let m = metrics.expect("Metrics should be available");
        assert_eq!(m.total_requests, 1);
        assert_eq!(m.success_count, 1);
        assert_eq!(m.avg_latency_ms, 150.0);

        // Verify health was updated (should be healthy)
        let health_status = router.health().get_status("cred-1").await;
        assert_eq!(
            health_status,
            crate::health::HealthStatus::Healthy,
            "Health should be healthy after success"
        );

        // Verify bandit policy was updated
        let stats = router
            .bandit_policy()
            .lock()
            .await
            .get_stats("cred-1")
            .cloned();
        assert!(stats.is_some(), "Bandit stats should be recorded");
        let s = stats.expect("Route statistics should be available");
        assert_eq!(s.pulls, 1);
        assert_eq!(s.last_utility, 0.9);
    }

    #[tokio::test]
    async fn test_router_plan_with_all_credentials_unavailable() {
        let mut router = Router::new();
        router.add_credential("cred-1".to_string(), vec!["laude-3-opus".to_string()]);
        router.set_model(
            "laude-3-opus".to_string(),
            create_test_model("laude-3-opus", "anthropic", 200000),
        );

        let request = create_test_request(1000);
        // Create unavailable auth
        let mut auth = create_test_auth("cred-1");
        auth.unavailable = true;
        let auths = vec![auth];

        router.metrics().initialize_auth("cred-1").await;

        let plan = router.plan(&request, auths, None).await;

        // Should still get a primary candidate (unavailable affects weight, not filtering)
        // The candidate builder doesn't filter by unavailable status
        assert!(
            plan.primary.is_some() || plan.filtered_candidates == 0,
            "Router should handle unavailable auths"
        );
    }

    #[tokio::test]
    async fn test_router_bandit_disabled_uses_weighted_selection() {
        let config = RouterConfig {
            use_bandit: false,
            ..Default::default()
        };
        let mut router = Router::with_config(config);
        router.add_credential("cred-1".to_string(), vec!["laude-3-opus".to_string()]);
        router.set_model(
            "laude-3-opus".to_string(),
            create_test_model("laude-3-opus", "anthropic", 200000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1")];

        router.metrics().initialize_auth("cred-1").await;

        let plan = router.plan(&request, auths, None).await;

        assert!(
            plan.primary.is_some(),
            "Should select route with bandit disabled"
        );
        let primary = plan
            .primary
            .expect("Execution plan should have a primary route");
        // With weighted selection (no bandit), utility determines selection
        assert!(primary.utility >= 0.0);
    }

    #[tokio::test]
    async fn test_router_session_affinity_disabled_uses_normal_selection() {
        let config = RouterConfig {
            enable_session_affinity: false,
            use_bandit: false, // Use weighted for deterministic testing
            ..Default::default()
        };
        let mut router = Router::with_config(config);
        router.add_credential("cred-1".to_string(), vec!["laude-3-opus".to_string()]);
        router.add_credential("cred-2".to_string(), vec!["gpt-4".to_string()]);
        router.set_model(
            "laude-3-opus".to_string(),
            create_test_model("laude-3-opus", "anthropic", 200000),
        );
        router.set_model(
            "gpt-4".to_string(),
            create_test_model("gpt-4", "openai", 128000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1"), create_test_auth("cred-2")];

        for auth in &auths {
            router.metrics().initialize_auth(&auth.id).await;
        }

        // Plan with session_id - should be ignored since affinity disabled
        let plan = router
            .plan(&request, auths.clone(), Some("test-session"))
            .await;

        assert!(
            plan.primary.is_some(),
            "Should select a route even with session affinity disabled"
        );
    }

    #[tokio::test]
    async fn test_select_weighted_handles_nan_scores_without_panic() {
        let config = RouterConfig {
            use_bandit: false, // Force weighted selection
            ..Default::default()
        };
        let mut router = Router::with_config(config);
        router.add_credential("cred-1".to_string(), vec!["model-1".to_string()]);
        router.add_credential("cred-2".to_string(), vec!["model-2".to_string()]);
        router.set_model(
            "model-1".to_string(),
            create_test_model("model-1", "provider-a", 200000),
        );
        router.set_model(
            "model-2".to_string(),
            create_test_model("model-2", "provider-b", 200000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1"), create_test_auth("cred-2")];

        router.metrics().initialize_auth("cred-1").await;
        router.metrics().initialize_auth("cred-2").await;

        // Record a zero-latency result to potentially produce NaN in utility calculation
        router.record_result("cred-1", true, 0.0, 200, 0.0).await;
        router.record_result("cred-2", true, 0.0, 200, 0.0).await;

        // This should NOT panic even if utility scores contain NaN
        let plan = router.plan(&request, auths, None).await;

        // Should still produce a valid plan
        assert!(
            plan.primary.is_some(),
            "Should select a route even with potential NaN scores"
        );
    }
}

mod session_affinity {
    use super::*;

    #[tokio::test]
    async fn test_session_affinity_records_provider_after_routing() {
        let config = RouterConfig {
            use_bandit: false,
            enable_session_affinity: true,
            ..Default::default()
        };
        let mut router = Router::with_config(config);
        router.add_credential("cred-1".to_string(), vec!["model-a".to_string()]);
        router.set_model(
            "model-a".to_string(),
            create_test_model("model-a", "provider-a", 200000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1")];

        router.metrics().initialize_auth("cred-1").await;

        let session_id = "affinity-test-session";

        // Before planning, no affinity exists
        assert!(
            !router.session_manager().has_affinity(session_id).await,
            "Should have no affinity before first plan call"
        );

        // Plan with session_id
        let plan = router.plan(&request, auths, Some(session_id)).await;
        assert!(plan.primary.is_some());

        // After planning, affinity should be recorded
        assert!(
            router.session_manager().has_affinity(session_id).await,
            "Should record session affinity after plan with session_id"
        );

        let preferred = router
            .session_manager()
            .get_preferred_provider(session_id)
            .await;
        assert_eq!(
            preferred,
            Some("provider-a".to_string()),
            "Should record the selected provider in session affinity"
        );
    }

    #[tokio::test]
    async fn test_session_affinity_prefers_cached_provider_over_higher_utility() {
        let config = RouterConfig {
            use_bandit: false,
            enable_session_affinity: true,
            ..Default::default()
        };
        let mut router = Router::with_config(config);
        router.add_credential("cred-1".to_string(), vec!["model-a".to_string()]);
        router.add_credential("cred-2".to_string(), vec!["model-b".to_string()]);
        router.set_model(
            "model-a".to_string(),
            create_test_model("model-a", "provider-a", 128000),
        );
        router.set_model(
            "model-b".to_string(),
            create_test_model("model-b", "provider-b", 200000),
        );

        let request = create_test_request(1000);
        let auths = vec![create_test_auth("cred-1"), create_test_auth("cred-2")];

        router.metrics().initialize_auth("cred-1").await;
        router.metrics().initialize_auth("cred-2").await;

        let session_id = "sticky-session";

        // Seed session affinity with provider-a (lower utility due to smaller context)
        router
            .session_manager()
            .set_provider(session_id.to_string(), "provider-a".to_string())
            .await
            .expect("Operation should succeed during test");

        // Plan with session: should prefer provider-a despite provider-b having higher utility
        let plan = router.plan(&request, auths, Some(session_id)).await;
        assert!(plan.primary.is_some());
        let primary = plan
            .primary
            .expect("Execution plan should have a primary route");

        assert_eq!(
            primary.provider, "provider-a",
            "Session affinity should prefer cached provider over higher utility alternative"
        );
    }
}
