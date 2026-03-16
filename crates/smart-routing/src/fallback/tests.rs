use super::*;
use crate::config::WeightConfig;
use crate::health::HealthManager;
use crate::metrics::MetricsCollector;
use crate::weight::AuthInfo;
use crate::weight::DefaultWeightCalculator;
use std::collections::HashSet;

fn create_test_auth(id: &str, provider: Option<&str>) -> AuthInfo {
    // If provider is specified, format the ID as "provider-key"
    let id = if let Some(p) = provider {
        format!("{p}-{id}")
    } else {
        id.to_string()
    };

    AuthInfo {
        id,
        priority: Some(0),
        quota_exceeded: false,
        unavailable: false,
        model_states: Vec::new(),
    }
}

mod fallback_generation {
    use super::*;

    #[tokio::test]
    async fn test_generate_fallbacks_basic() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("auth1", Some("anthropic")),
            create_test_auth("auth2", Some("openai")),
            create_test_auth("auth3", Some("anthropic")),
        ];

        // Initialize metrics
        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        assert!(!fallbacks.is_empty());
        assert_eq!(fallbacks[0].position, 0);
    }

    #[tokio::test]
    async fn test_generate_fallbacks_with_primary() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("auth1", Some("anthropic")),
            create_test_auth("auth2", Some("openai")),
            create_test_auth("auth3", Some("google")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let primary_id = auths[1].id.clone();
        let fallbacks = planner
            .generate_fallbacks(
                auths,
                Some(primary_id.clone()),
                &calculator,
                &metrics,
                &health,
            )
            .await;

        assert!(!fallbacks.is_empty());
        assert_eq!(fallbacks[0].auth_id, primary_id);
        assert_eq!(fallbacks[0].position, 0);
    }

    #[tokio::test]
    async fn test_empty_auths_returns_empty() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths: Vec<AuthInfo> = vec![];

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        assert!(fallbacks.is_empty());
    }

    #[tokio::test]
    async fn test_different_auth_credentials_for_fallbacks() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("key1", Some("anthropic")),
            create_test_auth("key2", Some("anthropic")),
            create_test_auth("key3", Some("openai")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        // All fallbacks should have different auth IDs
        let auth_ids: HashSet<_> = fallbacks.iter().map(|f| f.auth_id.clone()).collect();
        assert_eq!(auth_ids.len(), fallbacks.len());
    }
}

mod provider_diversity {
    use super::*;

    #[tokio::test]
    async fn test_provider_diversity() {
        let config = FallbackConfig {
            max_fallbacks: 10,
            min_fallbacks: 2,
            enable_provider_diversity: true,
            prefer_diverse_providers: true,
        };
        let planner = FallbackPlanner::with_config(config);
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("key1", Some("anthropic")),
            create_test_auth("key2", Some("anthropic")),
            create_test_auth("key3", Some("openai")),
            create_test_auth("key4", Some("google")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        // First two should be from different providers (highest weight from each)
        let providers: Vec<_> = fallbacks
            .iter()
            .filter_map(|f| f.provider.as_ref())
            .collect();

        if providers.len() >= 2 {
            // Check that first two are from different providers if possible
            assert_ne!(providers[0], providers[1]);
        }
    }

    #[tokio::test]
    async fn test_provider_diversity_all_same_provider() {
        let config = FallbackConfig {
            max_fallbacks: 5,
            min_fallbacks: 2,
            enable_provider_diversity: true,
            prefer_diverse_providers: true,
        };
        let planner = FallbackPlanner::with_config(config);
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        // All auths from same provider
        let auths = vec![
            create_test_auth("key1", Some("anthropic")),
            create_test_auth("key2", Some("anthropic")),
            create_test_auth("key3", Some("anthropic")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        // Should still return fallbacks, just all from same provider
        assert_eq!(
            fallbacks.len(),
            3,
            "Should return all auths even with same provider"
        );

        // All should be from anthropic
        for f in &fallbacks {
            assert_eq!(
                f.provider,
                Some("anthropic".to_string()),
                "All should be from anthropic"
            );
        }
    }

    #[tokio::test]
    async fn test_primary_auth_not_in_available_list() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("auth1", Some("anthropic")),
            create_test_auth("auth2", Some("openai")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        // Primary that doesn't exist in available auths
        let fallbacks = planner
            .generate_fallbacks(
                auths,
                Some("non-existent-primary".to_string()),
                &calculator,
                &metrics,
                &health,
            )
            .await;

        // Should still return available auths, just won't have primary first
        assert_eq!(fallbacks.len(), 2);
        // First won't be the non-existent primary
        assert_ne!(fallbacks[0].auth_id, "non-existent-primary");
    }
}

mod limits_and_filtering {
    use super::*;

    #[tokio::test]
    async fn test_max_fallbacks_limit() {
        let config = FallbackConfig {
            max_fallbacks: 2,
            min_fallbacks: 1,
            enable_provider_diversity: false,
            prefer_diverse_providers: false,
        };
        let planner = FallbackPlanner::with_config(config);
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("auth1", Some("anthropic")),
            create_test_auth("auth2", Some("openai")),
            create_test_auth("auth3", Some("google")),
            create_test_auth("auth4", Some("cohere")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        assert!(fallbacks.len() <= 2);
    }

    #[tokio::test]
    async fn test_unavailable_auths_filtered() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let mut auth1 = create_test_auth("auth1", Some("anthropic"));
        auth1.unavailable = true;

        let auth2 = create_test_auth("auth2", Some("openai"));

        let auths = vec![auth1, auth2];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        // Only auth2 should be in fallbacks (auth1 is unavailable)
        assert_eq!(fallbacks.len(), 1);
        assert!(fallbacks[0].auth_id.contains("openai"));
    }

    #[tokio::test]
    async fn test_limited_candidates_min_fallbacks() {
        let config = FallbackConfig {
            max_fallbacks: 5,
            min_fallbacks: 2,
            enable_provider_diversity: false,
            prefer_diverse_providers: false,
        };
        let planner = FallbackPlanner::with_config(config);
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        // Only 1 available auth
        let auths = vec![create_test_auth("auth1", Some("anthropic"))];

        metrics.initialize_auth(&auths[0].id).await;

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        // Should return what's available (1), even though min_fallbacks is 2
        assert_eq!(fallbacks.len(), 1);
    }

    #[tokio::test]
    async fn test_min_fallbacks_greater_than_available_auths() {
        let config = FallbackConfig {
            max_fallbacks: 10,
            min_fallbacks: 5, // More than available
            enable_provider_diversity: false,
            prefer_diverse_providers: false,
        };
        let planner = FallbackPlanner::with_config(config);
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        // Only 2 available auths
        let auths = vec![
            create_test_auth("auth1", Some("anthropic")),
            create_test_auth("auth2", Some("openai")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        // Should return what's available (2), not min_fallbacks (5)
        assert_eq!(
            fallbacks.len(),
            2,
            "Should return available auths when min_fallbacks > available"
        );
    }

    #[tokio::test]
    async fn test_max_fallbacks_zero_returns_empty() {
        let config = FallbackConfig {
            max_fallbacks: 0,
            min_fallbacks: 0,
            enable_provider_diversity: false,
            prefer_diverse_providers: false,
        };
        let planner = FallbackPlanner::with_config(config);
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("auth1", Some("anthropic")),
            create_test_auth("auth2", Some("openai")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        assert!(
            fallbacks.is_empty(),
            "max_fallbacks=0 should return empty list"
        );
    }

    #[tokio::test]
    async fn test_fallback_with_all_unavailable_auths() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        // All auths marked unavailable
        let mut auth1 = create_test_auth("auth1", Some("anthropic"));
        auth1.unavailable = true;
        let mut auth2 = create_test_auth("auth2", Some("openai"));
        auth2.unavailable = true;

        let auths = vec![auth1, auth2];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        assert!(
            fallbacks.is_empty(),
            "All unavailable auths should result in empty fallbacks"
        );
    }
}

mod ordering {
    use super::*;

    #[tokio::test]
    async fn test_fallback_ordering_by_weight() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("low_priority", Some("anthropic")),
            create_test_auth("high_priority", Some("openai")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        // Fallbacks should be ordered
        for (i, f) in fallbacks.iter().enumerate() {
            assert_eq!(f.position, i);
        }

        // Weights should be non-increasing
        for window in fallbacks.windows(2) {
            assert!(
                window[0].weight >= window[1].weight,
                "Weights should be non-increasing: {} >= {}",
                window[0].weight,
                window[1].weight
            );
        }
    }

    #[tokio::test]
    async fn test_fallback_ordering_weight_descending() {
        let planner = FallbackPlanner::new();
        let calculator = DefaultWeightCalculator::new(WeightConfig::default());
        let metrics = MetricsCollector::new();
        let health = HealthManager::new(crate::config::HealthConfig::default());

        let auths = vec![
            create_test_auth("low", Some("anthropic")),
            create_test_auth("high", Some("openai")),
            create_test_auth("mid", Some("google")),
        ];

        for auth in &auths {
            metrics.initialize_auth(&auth.id).await;
        }

        let fallbacks = planner
            .generate_fallbacks(auths, None, &calculator, &metrics, &health)
            .await;

        // Weights should be non-increasing (descending order)
        for window in fallbacks.windows(2) {
            assert!(
                window[0].weight >= window[1].weight,
                "Weights should be in descending order: {} >= {}",
                window[0].weight,
                window[1].weight
            );
        }
    }
}

mod extract_provider {
    use super::*;

    #[tokio::test]
    async fn test_extract_provider() {
        // Test various formats
        assert_eq!(
            FallbackPlanner::extract_provider("anthropic-key"),
            Some("anthropic".to_string())
        );
        assert_eq!(
            FallbackPlanner::extract_provider("openai-key-123"),
            Some("openai".to_string())
        );
        assert_eq!(
            FallbackPlanner::extract_provider("google_model_key"),
            Some("google".to_string())
        );
        assert_eq!(
            FallbackPlanner::extract_provider("cohere:key"),
            Some("cohere".to_string())
        );
        assert_eq!(FallbackPlanner::extract_provider(""), None);
    }

    #[test]
    fn test_extract_provider_multi_word_providers() {
        // amazon-bedrock should be recognized as a single provider, not "amazon"
        assert_eq!(
            FallbackPlanner::extract_provider("amazon-bedrock-us-east-1-key"),
            Some("amazon-bedrock".to_string())
        );
        assert_eq!(
            FallbackPlanner::extract_provider("azure-openai-gpt4-key"),
            Some("azure-openai".to_string())
        );
    }

    #[test]
    fn test_extract_provider_nested_paths() {
        // Standard single-segment providers still work
        assert_eq!(
            FallbackPlanner::extract_provider("deepseek-key"),
            Some("deepseek".to_string())
        );
        assert_eq!(
            FallbackPlanner::extract_provider("xai-grok-key"),
            Some("xai".to_string())
        );
    }

    #[test]
    fn test_extract_provider_unknown_prefix_falls_back() {
        // Unknown provider falls back to first segment
        assert_eq!(
            FallbackPlanner::extract_provider("my-custom-provider-key"),
            Some("my".to_string())
        );
        assert_eq!(
            FallbackPlanner::extract_provider("single"),
            Some("single".to_string())
        );
    }
}
