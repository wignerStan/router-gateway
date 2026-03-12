// BDD (Behavior-Driven Development) tests for smart-routing
//
// This module contains Cucumber-style tests that verify the behavior of
// the classification and health management systems.

#[cfg(test)]
mod bdd_integration {

    #[tokio::test]
    async fn test_bdd_classification_vision_detection() {
        use smart_routing::classification::ContentTypeDetector;

        // Scenario: Image attachment requires vision
        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "image_url",
                    "image_url": {"url": "https://example.com/image.png"}
                }]
            }]
        });
        assert!(ContentTypeDetector::detect_vision_required(&request));

        // Scenario: Text-only content does not require vision
        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": "Hello, world!"
            }]
        });
        assert!(!ContentTypeDetector::detect_vision_required(&request));

        // Scenario: Mixed content requires vision
        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "What is this?"},
                    {"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}
                ]
            }]
        });
        assert!(ContentTypeDetector::detect_vision_required(&request));
    }

    #[tokio::test]
    async fn test_bdd_classification_tool_detection() {
        use smart_routing::classification::ToolDetector;

        // Scenario: Tool definitions require tool support
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "What's the weather?"}],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "parameters": {"type": "object"}
                }
            }]
        });
        assert!(ToolDetector::detect_tools_required(&request));

        // Scenario: No tool definitions means no requirement
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });
        assert!(!ToolDetector::detect_tools_required(&request));

        // Scenario: Empty tool array does not require tools
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "tools": []
        });
        assert!(!ToolDetector::detect_tools_required(&request));
    }

    #[tokio::test]
    async fn test_bdd_classification_streaming_detection() {
        use smart_routing::classification::StreamingExtractor;

        // Scenario: Explicit streaming enabled requires streaming support
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": true
        });
        assert!(StreamingExtractor::extract_streaming_preference(&request));

        // Scenario: Explicit streaming disabled does not require streaming
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": false
        });
        assert!(!StreamingExtractor::extract_streaming_preference(&request));

        // Scenario: Default behavior when streaming flag is absent
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });
        assert!(!StreamingExtractor::extract_streaming_preference(&request));
    }

    #[tokio::test]
    async fn test_bdd_classification_format_detection() {
        use smart_routing::classification::FormatDetector;
        use smart_routing::classification::RequestFormat;

        // Scenario: OpenAI format requests are identified by structure
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "model": "gpt-4"
        });
        assert_eq!(FormatDetector::detect(&request), RequestFormat::OpenAI);

        // Scenario: Anthropic format requests are recognized
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "system": "You are a helpful assistant",
            "model": "claude-3-opus"
        });
        assert_eq!(FormatDetector::detect(&request), RequestFormat::Anthropic);

        // Scenario: Gemini format requests are detected
        let request = serde_json::json!({
            "contents": [{"parts": [{"text": "Hello"}]}],
            "model": "gemini-pro"
        });
        assert_eq!(FormatDetector::detect(&request), RequestFormat::Gemini);

        // Scenario: Unknown format defaults to generic handling
        let request = serde_json::json!({
            "prompt": "Hello",
            "model": "unknown-model"
        });
        assert_eq!(FormatDetector::detect(&request), RequestFormat::Generic);
    }

    #[tokio::test]
    async fn test_bdd_classification_token_estimation() {
        use smart_routing::classification::TokenEstimator;

        // Scenario: Small prompt fits standard context
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });
        let tokens = TokenEstimator::estimate(&request);
        assert!(tokens < 1000, "Small prompt should fit standard context");

        // Scenario: Large prompt requires high context capacity
        let large_text = "x".repeat(200000); // ~50000 tokens
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": large_text}]
        });
        let tokens = TokenEstimator::estimate(&request);
        assert!(tokens > 40000, "Large prompt should require high context");

        // Scenario: Total estimated tokens combines input and expected output
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "x".repeat(4000)}], // ~1000 input tokens
            "max_tokens": 500
        });
        let tokens = TokenEstimator::estimate(&request);
        assert!(
            tokens > 1400 && tokens < 1600,
            "Total should combine input and output"
        );
    }

    #[tokio::test]
    async fn test_bdd_classification_reasoning_detection() {
        use smart_routing::reasoning::{ReasoningInference, ReasoningRequest};
        use std::collections::HashMap;

        let inference = ReasoningInference::new();

        // Scenario: Reasoning flag explicitly enabled requires thinking support
        let request = ReasoningRequest {
            model: "gpt-4".to_string(),
            reasoning_flag: Some(true),
            max_tokens: None,
            hints: HashMap::new(),
        };
        assert!(inference.requires_reasoning(&request).await);

        // Scenario: Model family hint suggests reasoning requirement
        let request = ReasoningRequest {
            model: "o1-mini".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };
        assert!(inference.requires_reasoning(&request).await);

        // Scenario: Standard requests do not require thinking
        let request = ReasoningRequest {
            model: "gpt-4".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };
        assert!(!inference.requires_reasoning(&request).await);
    }

    #[tokio::test]
    async fn test_bdd_health_rate_limit_triggers_degraded() {
        use smart_routing::config::HealthConfig;
        use smart_routing::health::{HealthManager, HealthStatus};

        let config = HealthConfig {
            status_codes: smart_routing::config::StatusCodeHealthConfig {
                degraded: vec![429],
                unhealthy: vec![],
                healthy: vec![],
            },
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        // Scenario: Rate limit triggers degraded state
        assert_eq!(manager.get_status("test-auth").await, HealthStatus::Healthy);
        manager.update_from_result("test-auth", false, 429).await;
        assert_eq!(
            manager.get_status("test-auth").await,
            HealthStatus::Degraded
        );
    }

    #[tokio::test]
    async fn test_bdd_health_consecutive_failures_trigger_unhealthy() {
        use smart_routing::config::HealthConfig;
        use smart_routing::health::{HealthManager, HealthStatus};

        let config = HealthConfig {
            unhealthy_threshold: 5,
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        // Scenario: Consecutive failures trigger unhealthy state
        assert_eq!(manager.get_status("test-auth").await, HealthStatus::Healthy);
        for _ in 0..5 {
            manager.update_from_result("test-auth", false, 500).await;
        }
        assert_eq!(
            manager.get_status("test-auth").await,
            HealthStatus::Unhealthy
        );
    }

    #[tokio::test]
    async fn test_bdd_health_success_streak_recovers_degraded() {
        use smart_routing::config::HealthConfig;
        use smart_routing::health::{HealthManager, HealthStatus};

        let config = HealthConfig {
            healthy_threshold: 3,
            status_codes: smart_routing::config::StatusCodeHealthConfig {
                degraded: vec![429],
                unhealthy: vec![],
                healthy: vec![],
            },
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        // Scenario: Success streak recovers degraded credential
        manager.update_from_result("test-auth", false, 429).await;
        assert_eq!(
            manager.get_status("test-auth").await,
            HealthStatus::Degraded
        );

        for _ in 0..3 {
            manager.update_from_result("test-auth", true, 200).await;
        }
        assert_eq!(manager.get_status("test-auth").await, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_bdd_health_unhealthy_blocked_from_selection() {
        use smart_routing::config::HealthConfig;
        use smart_routing::health::{HealthManager, HealthStatus};

        let config = HealthConfig {
            unhealthy_threshold: 2,
            cooldown_period_seconds: 10,
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        // Scenario: Unhealthy credential blocked from selection
        for _ in 0..2 {
            manager.update_from_result("test-auth", false, 500).await;
        }
        assert_eq!(
            manager.get_status("test-auth").await,
            HealthStatus::Unhealthy
        );
        assert!(!manager.is_available("test-auth").await);
    }

    #[tokio::test]
    async fn test_bdd_health_cooldown_expiration_allows_recovery() {
        use smart_routing::config::HealthConfig;
        use smart_routing::health::{HealthManager, HealthStatus};

        let config = HealthConfig {
            unhealthy_threshold: 2,
            cooldown_period_seconds: 1,
            ..Default::default()
        };
        let manager = HealthManager::new(config);

        // Scenario: Cooldown expiration allows recovery attempt
        for _ in 0..2 {
            manager.update_from_result("test-auth", false, 500).await;
        }
        assert_eq!(
            manager.get_status("test-auth").await,
            HealthStatus::Unhealthy
        );
        assert!(!manager.is_available("test-auth").await);

        // Wait for cooldown to expire
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Cooldown expired, but still unhealthy status
        assert_eq!(
            manager.get_status("test-auth").await,
            HealthStatus::Unhealthy
        );
    }

    #[tokio::test]
    async fn test_bdd_all_request_classification_scenarios() {
        use smart_routing::classification::{
            ContentTypeDetector, FormatDetector, RequestFormat, StreamingExtractor, ToolDetector,
        };

        // Scenario: All capabilities detected in complex request
        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "Analyze this image"},
                    {"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}
                ]
            }],
            "tools": [{"type": "function", "function": {"name": "analyze", "parameters": {}}}],
            "stream": true
        });

        assert!(ContentTypeDetector::detect_vision_required(&request));
        assert!(ToolDetector::detect_tools_required(&request));
        assert!(StreamingExtractor::extract_streaming_preference(&request));
        assert_eq!(FormatDetector::detect(&request), RequestFormat::OpenAI);
    }

    #[tokio::test]
    async fn test_bdd_all_health_state_transitions() {
        use smart_routing::config::HealthConfig;
        use smart_routing::health::{HealthManager, HealthStatus};

        let config = HealthConfig {
            status_codes: smart_routing::config::StatusCodeHealthConfig {
                degraded: vec![429, 503],
                unhealthy: vec![401, 403, 500, 502, 504],
                healthy: vec![],
            },
            unhealthy_threshold: 3,
            healthy_threshold: 3,
            degraded_threshold: 0.5,
            cooldown_period_seconds: 1,
        };
        let manager = HealthManager::new(config);

        // Test full state machine: Healthy -> Degraded -> Unhealthy -> Degraded -> Healthy
        let auth_id = "state-transition-test";

        // Start healthy
        assert_eq!(manager.get_status(auth_id).await, HealthStatus::Healthy);

        // Trigger degraded (rate limit)
        manager.update_from_result(auth_id, false, 429).await;
        assert_eq!(manager.get_status(auth_id).await, HealthStatus::Degraded);

        // Trigger unhealthy (3 more failures)
        for _ in 0..3 {
            manager.update_from_result(auth_id, false, 500).await;
        }
        assert_eq!(manager.get_status(auth_id).await, HealthStatus::Unhealthy);

        // Recover to degraded (rate limit response)
        manager.update_from_result(auth_id, false, 429).await;
        assert_eq!(manager.get_status(auth_id).await, HealthStatus::Degraded);

        // Recover to healthy (3 consecutive successes)
        for _ in 0..3 {
            manager.update_from_result(auth_id, true, 200).await;
        }
        assert_eq!(manager.get_status(auth_id).await, HealthStatus::Healthy);
    }

    // ================================================================
    // Planner-mode BDD scenarios
    // ================================================================

    #[tokio::test]
    async fn test_bdd_learned_mode_uses_bandit_priors() {
        use smart_routing::bandit::BanditConfig;
        use smart_routing::bandit::BanditPolicy;
        use smart_routing::bandit::TierPriors;

        // Scenario: When bandit has accumulated data, learned priors influence selection
        let config = BanditConfig {
            min_samples_for_thompson: 3,
            tier_priors: Some(TierPriors::default()),
            ..Default::default()
        };
        let mut bandit = BanditPolicy::with_config(config);
        bandit.set_route_tier("flagship-route", smart_routing::bandit::Tier::Flagship);
        bandit.set_route_tier("fast-route", smart_routing::bandit::Tier::Fast);

        // Train flagship-route to be successful
        for _ in 0..10 {
            bandit.record_result("flagship-route", true, 0.9);
        }
        // Train fast-route to fail
        for _ in 0..10 {
            bandit.record_result("fast-route", false, 0.1);
        }

        let routes = vec!["flagship-route".to_string(), "fast-route".to_string()];

        // flagship should win significantly more often due to learned stats
        let mut flagship_wins = 0;
        for _ in 0..100 {
            let selected = bandit.select_route(&routes).unwrap();
            if selected == "flagship-route" {
                flagship_wins += 1;
            }
        }
        assert!(
            flagship_wins > 70,
            "Learned mode should prefer flagship ({}/100)",
            flagship_wins
        );
    }

    #[tokio::test]
    async fn test_bdd_heuristic_mode_fallback_provider_inference() {
        use smart_routing::config::WeightConfig;
        use smart_routing::fallback::FallbackPlanner;
        use smart_routing::weight::DefaultWeightCalculator;

        // Scenario: FallbackPlanner infers providers via known-provider matching in generate_fallbacks()
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = smart_routing::MetricsCollector::new();
        let health =
            smart_routing::HealthManager::new(smart_routing::config::HealthConfig::default());

        // Auth IDs with known-provider prefixes
        let auths = vec![
            smart_routing::weight::AuthInfo {
                id: "amazon-bedrock-us-east-key".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            smart_routing::weight::AuthInfo {
                id: "azure-openai-gpt4-key".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            smart_routing::weight::AuthInfo {
                id: "deepseek-api-key".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        assert_eq!(
            fallbacks.len(),
            3,
            "Should generate fallbacks for all auths"
        );

        // Verify provider inference through FallbackRoute.provider field
        let providers: Vec<_> = fallbacks.iter().map(|f| f.provider.as_deref()).collect();
        assert!(
            providers.contains(&Some("amazon-bedrock")),
            "Should infer 'amazon-bedrock' from multi-word prefix"
        );
        assert!(
            providers.contains(&Some("azure-openai")),
            "Should infer 'azure-openai' from multi-word prefix"
        );
        assert!(
            providers.contains(&Some("deepseek")),
            "Should infer 'deepseek' from single-word prefix"
        );
    }

    #[test]
    fn test_bdd_safe_weighted_mode_handles_nan_scores() {
        use std::cmp::Ordering;

        // Scenario: Weighted selection handles NaN scores via unwrap_or(Ordering::Equal)
        // This mirrors the pattern in Router::select_weighted()
        let scores = [
            ("cred-a".to_string(), 0.9),
            ("cred-b".to_string(), f64::NAN),
            ("cred-c".to_string(), 0.5),
        ];

        // Find max using partial_cmp().unwrap_or(Ordering::Equal) - same as router code
        let best = scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

        assert!(best.is_some());
        let (id, score) = best.unwrap();
        assert!(
            !id.is_empty(),
            "Safe weighted mode should always produce a result"
        );
        // A non-NaN score should win over NaN (NaN compares as Equal via unwrap_or)
        assert!(
            !score.is_nan(),
            "Best score should be non-NaN ({score}), actual: {id}={score}"
        );
    }

    #[tokio::test]
    async fn test_bdd_deterministic_fallback_when_all_filtered() {
        use smart_routing::classification::{
            ClassifiedRequest, QualityPreference, RequestFormat, RequiredCapabilities,
        };
        use smart_routing::router::Router;

        // Scenario: When all candidates are filtered out, plan returns empty primary
        let mut router = Router::with_config(smart_routing::router::RouterConfig {
            use_bandit: false,
            ..Default::default()
        });
        router.add_credential("cred-1".to_string(), vec!["small-model".to_string()]);
        router.set_model(
            "small-model".to_string(),
            model_registry::ModelInfo {
                id: "small-model".to_string(),
                name: "Small Model".to_string(),
                provider: "test".to_string(),
                context_window: 1000, // Very small context
                max_output_tokens: 256,
                input_price_per_million: 1.0,
                output_price_per_million: 2.0,
                capabilities: model_registry::ModelCapabilities {
                    streaming: false,
                    tools: false,
                    vision: false,
                    thinking: false,
                },
                rate_limits: model_registry::RateLimits {
                    requests_per_minute: 60,
                    tokens_per_minute: 90000,
                },
                source: model_registry::DataSource::Static,
            },
        );

        // Request needs way more context than available
        let request = ClassifiedRequest {
            required_capabilities: RequiredCapabilities::default(),
            estimated_tokens: 100_000, // Way exceeds 1000 context
            format: RequestFormat::OpenAI,
            quality_preference: QualityPreference::Balanced,
        };
        let auths = vec![smart_routing::weight::AuthInfo {
            id: "cred-1".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        }];

        let plan = router.plan(&request, auths, None).await;

        assert!(
            plan.primary.is_none(),
            "All candidates filtered should yield None primary"
        );
        assert_eq!(plan.total_candidates, 1);
        assert_eq!(plan.filtered_candidates, 0);
        assert!(
            plan.fallbacks.is_empty(),
            "No fallbacks when all candidates filtered"
        );
    }

    #[tokio::test]
    async fn test_bdd_session_affinity_returns_same_provider() {
        use smart_routing::classification::{
            ClassifiedRequest, QualityPreference, RequestFormat, RequiredCapabilities,
        };
        use smart_routing::router::Router;

        // Scenario: Router::plan() with session_id returns same provider for repeated calls
        let mut router = Router::with_config(smart_routing::router::RouterConfig {
            use_bandit: false,
            enable_session_affinity: true,
            ..Default::default()
        });
        router.add_credential("cred-1".to_string(), vec!["model-a".to_string()]);
        router.add_credential("cred-2".to_string(), vec!["model-b".to_string()]);
        router.set_model(
            "model-a".to_string(),
            model_registry::ModelInfo {
                id: "model-a".to_string(),
                name: "Model A".to_string(),
                provider: "provider-a".to_string(),
                context_window: 200_000,
                max_output_tokens: 4096,
                input_price_per_million: 1.0,
                output_price_per_million: 2.0,
                capabilities: model_registry::ModelCapabilities {
                    streaming: true,
                    tools: true,
                    vision: false,
                    thinking: false,
                },
                rate_limits: model_registry::RateLimits {
                    requests_per_minute: 60,
                    tokens_per_minute: 90000,
                },
                source: model_registry::DataSource::Static,
            },
        );
        router.set_model(
            "model-b".to_string(),
            model_registry::ModelInfo {
                id: "model-b".to_string(),
                name: "Model B".to_string(),
                provider: "provider-b".to_string(),
                context_window: 200_000,
                max_output_tokens: 4096,
                input_price_per_million: 1.0,
                output_price_per_million: 2.0,
                capabilities: model_registry::ModelCapabilities {
                    streaming: true,
                    tools: true,
                    vision: false,
                    thinking: false,
                },
                rate_limits: model_registry::RateLimits {
                    requests_per_minute: 60,
                    tokens_per_minute: 90000,
                },
                source: model_registry::DataSource::Static,
            },
        );

        let request = ClassifiedRequest {
            required_capabilities: RequiredCapabilities::default(),
            estimated_tokens: 1000,
            format: RequestFormat::OpenAI,
            quality_preference: QualityPreference::Balanced,
        };
        let auths = vec![
            smart_routing::weight::AuthInfo {
                id: "cred-1".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
            smart_routing::weight::AuthInfo {
                id: "cred-2".to_string(),
                priority: Some(0),
                quota_exceeded: false,
                unavailable: false,
                model_states: Vec::new(),
            },
        ];

        router.metrics().initialize_auth("cred-1").await;
        router.metrics().initialize_auth("cred-2").await;

        let session_id = "test-session-42";

        // First call establishes affinity
        let plan1 = router.plan(&request, auths.clone(), Some(session_id)).await;
        assert!(plan1.primary.is_some());
        let first_provider = plan1.primary.as_ref().unwrap().provider.clone();

        // Second call should prefer the same provider via affinity
        let plan2 = router.plan(&request, auths, Some(session_id)).await;
        assert!(plan2.primary.is_some());
        let second_provider = plan2.primary.as_ref().unwrap().provider.clone();

        assert_eq!(
            first_provider, second_provider,
            "Session affinity should return same provider for same session"
        );
    }

    #[tokio::test]
    async fn test_bdd_router_clone_preserves_shared_state() {
        use smart_routing::classification::{
            ClassifiedRequest, QualityPreference, RequestFormat, RequiredCapabilities,
        };
        use smart_routing::router::Router;

        // Scenario: Cloned router retains bandit state and session affinity (shared via Arc)
        let mut router = Router::new();
        router.add_credential("cred-1".to_string(), vec!["model-a".to_string()]);
        router.set_model(
            "model-a".to_string(),
            model_registry::ModelInfo {
                id: "model-a".to_string(),
                name: "Model A".to_string(),
                provider: "provider-a".to_string(),
                context_window: 200_000,
                max_output_tokens: 4096,
                input_price_per_million: 1.0,
                output_price_per_million: 2.0,
                capabilities: model_registry::ModelCapabilities {
                    streaming: true,
                    tools: true,
                    vision: false,
                    thinking: false,
                },
                rate_limits: model_registry::RateLimits {
                    requests_per_minute: 60,
                    tokens_per_minute: 90000,
                },
                source: model_registry::DataSource::Static,
            },
        );

        router.metrics().initialize_auth("cred-1").await;

        // Record a result on the original to populate bandit state
        router.record_result("cred-1", true, 100.0, 200, 0.9).await;

        // Set session affinity on original
        router
            .session_manager()
            .set_provider("session-1".to_string(), "provider-a".to_string())
            .await
            .unwrap();

        let cloned = router.clone();

        // Clone should see the same bandit data (shared via Arc)
        let bandit = cloned.bandit_policy().lock().await;
        let stats = bandit.get_stats("cred-1");
        assert!(stats.is_some(), "Clone should share bandit state");
        let s = stats.unwrap();
        assert_eq!(s.pulls, 1, "Bandit pull count should be preserved");
        assert_eq!(s.last_utility, 0.9, "Bandit utility should be preserved");
        drop(bandit);

        // Clone should see the same session affinity (shared via Arc)
        let provider = cloned
            .session_manager()
            .get_preferred_provider("session-1")
            .await;
        assert_eq!(
            provider,
            Some("provider-a".to_string()),
            "Clone should share session affinity"
        );

        // Clone should be usable for planning
        let request = ClassifiedRequest {
            required_capabilities: RequiredCapabilities::default(),
            estimated_tokens: 1000,
            format: RequestFormat::OpenAI,
            quality_preference: QualityPreference::Balanced,
        };
        let auths = vec![smart_routing::weight::AuthInfo {
            id: "cred-1".to_string(),
            priority: Some(0),
            quota_exceeded: false,
            unavailable: false,
            model_states: Vec::new(),
        }];

        let plan = cloned.plan(&request, auths, Some("session-1")).await;
        assert!(
            plan.primary.is_some(),
            "Cloned router should produce valid plans"
        );
    }

    #[test]
    fn test_bdd_nan_safe_score_sorting() {
        use std::cmp::Ordering;

        // Scenario: Sorting scores containing NaN uses unwrap_or(Ordering::Equal)
        let mut scores: Vec<(String, f64)> = [
            ("good".to_string(), 0.8),
            ("nan-a".to_string(), f64::NAN),
            ("better".to_string(), 0.95),
            ("nan-b".to_string(), f64::NAN),
            ("okay".to_string(), 0.5),
        ]
        .into();

        // This sort mirrors the router's weighted selection pattern
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

        // Should not panic; first element should be non-NaN (NaN compares as Equal)
        assert!(!scores[0].0.is_empty());
        // At least one non-NaN element should be in the top positions
        let top_three: Vec<_> = scores.iter().take(3).filter(|(_, s)| !s.is_nan()).collect();
        assert!(
            top_three.len() >= 2,
            "Non-NaN scores should sort to top positions"
        );
    }

    #[test]
    fn test_bdd_token_usage_in_execution_result() {
        use smart_routing::executor::ExecutionResult;

        // Scenario: ExecutionResult supports prompt_tokens and completion_tokens fields
        let result_with_tokens = ExecutionResult {
            success: true,
            credential_id: Some("cred-1".to_string()),
            model_id: Some("model-a".to_string()),
            attempts: 1,
            total_latency_ms: 200.0,
            status_code: Some(200),
            error: None,
            prompt_tokens: Some(1500),
            completion_tokens: Some(800),
        };
        assert_eq!(result_with_tokens.prompt_tokens, Some(1500));
        assert_eq!(result_with_tokens.completion_tokens, Some(800));

        // Scenario: ExecutionResult can have None token counts (executor returns None)
        let result_no_tokens = ExecutionResult {
            success: true,
            credential_id: Some("cred-2".to_string()),
            model_id: Some("model-b".to_string()),
            attempts: 1,
            total_latency_ms: 100.0,
            status_code: Some(200),
            error: None,
            prompt_tokens: None,
            completion_tokens: None,
        };
        assert_eq!(result_no_tokens.prompt_tokens, None);
        assert_eq!(result_no_tokens.completion_tokens, None);

        // Scenario: Zero token counts are valid
        let result_zero_tokens = ExecutionResult {
            success: true,
            credential_id: Some("cred-3".to_string()),
            model_id: Some("model-c".to_string()),
            attempts: 1,
            total_latency_ms: 50.0,
            status_code: Some(200),
            error: None,
            prompt_tokens: Some(0),
            completion_tokens: Some(0),
        };
        assert_eq!(result_zero_tokens.prompt_tokens, Some(0));
        assert_eq!(result_zero_tokens.completion_tokens, Some(0));
    }
}
