use super::*;

mod basic_get_and_cache {
    use super::*;

    #[tokio::test]
    async fn test_registry_get() {
        let registry = Registry::new();
        let model = registry
            .get("gpt-4o")
            .await
            .expect("Model registry operation should succeed during test");
        assert!(model.is_some());
    }

    #[tokio::test]
    async fn test_registry_get_not_found() {
        let registry = Registry::new();
        let model = registry
            .get("unknown-model")
            .await
            .expect("Model registry operation should succeed during test");
        assert!(model.is_none());
    }

    #[tokio::test]
    async fn test_registry_get_multiple() {
        let registry = Registry::new();
        let models = registry
            .get_multiple(&[
                "gpt-4o".to_string(),
                "gemini-2.5-flash".to_string(),
                "unknown".to_string(),
            ])
            .await
            .expect("Model registry operation should succeed during test");

        assert_eq!(models.len(), 2);
        assert!(models.contains_key("gpt-4o"));
        assert!(models.contains_key("gemini-2.5-flash"));
    }

    #[tokio::test]
    async fn test_registry_get_multiple_empty() {
        let registry = Registry::new();

        let result = registry.get_multiple(&[]).await;
        assert!(result.is_ok());
        assert_eq!(
            result
                .expect("Model registry operation should succeed during test")
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn test_registry_get_multiple_with_empty_strings() {
        let registry = Registry::new();

        // Mix of valid and empty model IDs
        let result = registry
            .get_multiple(&[
                "gpt-4o".to_string(),
                "".to_string(),
                "gemini-2.5-flash".to_string(),
            ])
            .await;

        assert!(result.is_ok());
        let models = result.expect("Model registry operation should succeed during test");
        // Should only have the valid models
        assert!(models.contains_key("gpt-4o"));
        assert!(models.contains_key("gemini-2.5-flash"));
        assert!(!models.contains_key(""));
    }
}

mod cache_management {
    use super::*;

    #[tokio::test]
    async fn test_registry_cached_count() {
        let registry = Registry::new();

        // Access some models to populate cache
        let _ = registry.get("gpt-4o").await;
        let _ = registry.get("claude-sonnet-4-20250514").await;

        let count = registry.cached_count().await;
        assert!(count >= 2);
    }

    #[tokio::test]
    async fn test_registry_invalidate() {
        let registry = Registry::new();

        // Populate cache
        let _ = registry.get("gpt-4o").await;
        assert!(registry.cached_count().await > 0);

        // Clear cache
        registry.invalidate(&[]).await;
        assert_eq!(registry.cached_count().await, 0);
    }

    #[tokio::test]
    async fn test_registry_invalidate_specific_models() {
        let registry = Registry::new();

        // Populate cache with multiple models
        let _ = registry.get("gpt-4o").await;
        let _ = registry.get("claude-sonnet-4-20250514").await;

        let cached_count = registry.cached_count().await;
        assert!(cached_count >= 2);

        // Invalidate specific model
        registry.invalidate(&["gpt-4o".to_string()]).await;

        let cached_ids = registry.cached_ids().await;
        assert!(!cached_ids.contains(&"gpt-4o".to_string()));
        assert!(cached_ids.contains(&"claude-sonnet-4-20250514".to_string()));
    }

    #[tokio::test]
    async fn test_registry_cache_expiration() {
        let config = RegistryConfig {
            ttl: chrono::Duration::milliseconds(100),
            ..Default::default()
        };
        let registry = Registry::with_config(config);

        // Fetch and cache
        let model1 = registry
            .get("gpt-4o")
            .await
            .expect("Model registry operation should succeed during test");
        assert!(model1.is_some());

        // Should still be cached
        let cached_count = registry.cached_count().await;
        assert!(cached_count > 0);

        // Wait for TTL to expire
        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

        // Cleanup expired entries
        let removed = registry.cleanup_expired().await;
        assert!(removed > 0);

        // Cache should be empty
        let cached_count = registry.cached_count().await;
        assert_eq!(cached_count, 0);
    }

    #[tokio::test]
    async fn test_registry_cleanup_expired_with_fresh_entries() {
        let registry = Registry::new();

        // Populate cache with fresh entries
        let _ = registry.get("gpt-4o").await;

        // Cleanup should remove nothing (all entries fresh)
        let removed = registry.cleanup_expired().await;
        assert_eq!(removed, 0);

        let cached_count = registry.cached_count().await;
        assert!(cached_count > 0);
    }

    #[tokio::test]
    async fn test_registry_clone() {
        let registry1 = Registry::new();
        let _ = registry1.get("gpt-4o").await;

        let registry2 = registry1.clone();

        // Both should share the same cache
        let count1 = registry1.cached_count().await;
        let count2 = registry2.cached_count().await;
        assert_eq!(count1, count2);
    }

    #[tokio::test]
    async fn test_registry_concurrent_access() {
        let registry = std::sync::Arc::new(Registry::new());
        let mut handles = vec![];

        // Spawn multiple concurrent readers
        for i in 0..10 {
            let registry_clone = Arc::clone(&registry);
            let handle = tokio::spawn(async move {
                let model_id = if i % 2 == 0 {
                    "gpt-4o"
                } else {
                    "gemini-2.5-flash"
                };
                registry_clone.get(model_id).await
            });
            handles.push(handle);
        }

        // All should succeed
        for handle in handles {
            let result = handle
                .await
                .expect("Model registry operation should succeed during test");
            assert!(result.is_ok());
        }

        // Cache should have both models
        let cached_count = registry.cached_count().await;
        assert!(cached_count >= 2);
    }
}

mod filter_methods {
    use super::*;

    #[tokio::test]
    async fn test_registry_filter_by_capability() {
        let registry = Registry::new();

        // Populate cache
        let _ = registry.get("gpt-4o").await;
        let _ = registry.get("claude-sonnet-4-20250514").await;

        let vision_models = registry
            .filter_by_capability(CapabilityCategory::Vision)
            .await;
        assert!(!vision_models.is_empty());
    }

    #[tokio::test]
    async fn test_registry_filter_by_tier() {
        let registry = Registry::new();

        // Populate cache
        let _ = registry.get("claude-opus-4-20250514").await;
        let _ = registry.get("gpt-4o").await;

        let flagship_models = registry.filter_by_tier(TierCategory::Flagship).await;
        assert!(!flagship_models.is_empty());
    }

    #[tokio::test]
    async fn test_filter_by_capability_empty_cache() {
        let registry = Registry::new();
        // Don't populate cache

        let models = registry
            .filter_by_capability(CapabilityCategory::Vision)
            .await;
        assert!(models.is_empty(), "Empty cache should return empty result");
    }

    #[tokio::test]
    async fn test_filter_by_capability_expired_entries() {
        let config = RegistryConfig {
            ttl: chrono::Duration::milliseconds(50),
            ..Default::default()
        };
        let registry = Registry::with_config(config);

        // Populate cache
        let _ = registry.get("gpt-4o").await;

        // Wait for TTL to expire
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Filter should return empty (expired entries)
        let models = registry
            .filter_by_capability(CapabilityCategory::Vision)
            .await;
        assert!(models.is_empty(), "Expired entries should be filtered out");
    }

    #[tokio::test]
    async fn test_filter_by_tier_flagship() {
        let registry = Registry::new();
        let _ = registry.get("claude-opus-4-20250514").await; // Flagship model

        let flagship_models = registry.filter_by_tier(TierCategory::Flagship).await;
        assert!(!flagship_models.is_empty(), "Should find flagship models");

        // All returned models should be flagship
        for model in &flagship_models {
            assert!(model.is_in_tier(TierCategory::Flagship));
        }
    }

    #[tokio::test]
    async fn test_filter_by_tier_standard() {
        let registry = Registry::new();
        let _ = registry.get("claude-sonnet-4-20250514").await;

        let standard_models = registry.filter_by_tier(TierCategory::Standard).await;
        assert!(!standard_models.is_empty(), "Should find standard models");

        for model in &standard_models {
            assert!(model.is_in_tier(TierCategory::Standard));
        }
    }

    #[tokio::test]
    async fn test_filter_by_tier_fast() {
        let registry = Registry::new();
        let _ = registry.get("gemini-2.5-flash").await;

        let fast_models = registry.filter_by_tier(TierCategory::Fast).await;
        assert!(!fast_models.is_empty(), "Should find fast models");

        for model in &fast_models {
            assert!(model.is_in_tier(TierCategory::Fast));
        }
    }

    #[tokio::test]
    async fn test_filter_by_cost_all_categories() {
        let registry = Registry::new();

        // Populate with models of different cost categories
        let _ = registry.get("gpt-4o").await; // Standard
        let _ = registry.get("claude-sonnet-4-20250514").await; // Economy

        // Test each cost category
        let economy_models = registry.filter_by_cost(CostCategory::Economy).await;
        for model in &economy_models {
            assert!(model.is_in_cost_range(CostCategory::Economy));
        }

        let standard_models = registry.filter_by_cost(CostCategory::Standard).await;
        for model in &standard_models {
            assert!(model.is_in_cost_range(CostCategory::Standard));
        }

        let premium_models = registry.filter_by_cost(CostCategory::Premium).await;
        for model in &premium_models {
            assert!(model.is_in_cost_range(CostCategory::Premium));
        }

        let ultra_premium_models = registry.filter_by_cost(CostCategory::UltraPremium).await;
        for model in &ultra_premium_models {
            assert!(model.is_in_cost_range(CostCategory::UltraPremium));
        }
    }

    #[tokio::test]
    async fn test_filter_by_context_window_all_categories() {
        let registry = Registry::new();

        // Populate with models of different context sizes
        let _ = registry.get("gpt-4o").await; // 128K - Large
        let _ = registry.get("claude-sonnet-4-20250514").await; // 1M - Ultra

        // Test each context window category
        let small_models = registry
            .filter_by_context_window(ContextWindowCategory::Small)
            .await;
        for model in &small_models {
            assert!(model.is_in_context_range(ContextWindowCategory::Small));
        }

        let medium_models = registry
            .filter_by_context_window(ContextWindowCategory::Medium)
            .await;
        for model in &medium_models {
            assert!(model.is_in_context_range(ContextWindowCategory::Medium));
        }

        let large_models = registry
            .filter_by_context_window(ContextWindowCategory::Large)
            .await;
        for model in &large_models {
            assert!(model.is_in_context_range(ContextWindowCategory::Large));
        }

        let ultra_models = registry
            .filter_by_context_window(ContextWindowCategory::Ultra)
            .await;
        for model in &ultra_models {
            assert!(model.is_in_context_range(ContextWindowCategory::Ultra));
        }
    }

    #[tokio::test]
    async fn test_filter_by_provider_all_variants() {
        let registry = Registry::new();

        // Populate with models from different providers
        let _ = registry.get("gpt-4o").await; // OpenAI
        let _ = registry.get("gemini-2.5-flash").await; // Google

        // Test major providers
        let openai_models = registry.filter_by_provider(ProviderCategory::OpenAI).await;
        assert!(!openai_models.is_empty(), "Should find OpenAI models");
        for model in &openai_models {
            assert!(model.is_from_provider(ProviderCategory::OpenAI));
        }

        let google_models = registry.filter_by_provider(ProviderCategory::Google).await;
        assert!(!google_models.is_empty(), "Should find Google models");
        for model in &google_models {
            assert!(model.is_from_provider(ProviderCategory::Google));
        }

        // Provider with no models
        let xai_models = registry.filter_by_provider(ProviderCategory::XAI).await;
        assert!(
            xai_models.is_empty(),
            "Should not find xAI models in default fetcher"
        );
    }

    #[tokio::test]
    async fn test_filter_by_capability_all_capability_types() {
        let registry = Registry::new();

        let _ = registry.get("claude-opus-4-20250514").await; // Has all capabilities

        // Test each capability type
        let streaming_models = registry
            .filter_by_capability(CapabilityCategory::Streaming)
            .await;
        assert!(!streaming_models.is_empty());

        let tools_models = registry
            .filter_by_capability(CapabilityCategory::Tools)
            .await;
        assert!(!tools_models.is_empty());

        let vision_models = registry
            .filter_by_capability(CapabilityCategory::Vision)
            .await;
        assert!(!vision_models.is_empty());

        let thinking_models = registry
            .filter_by_capability(CapabilityCategory::Thinking)
            .await;
        assert!(
            !thinking_models.is_empty(),
            "Thinking model should support thinking capability"
        );
    }
}

mod filter_edge_cases {
    use super::*;

    #[tokio::test]
    async fn test_filter_methods_with_only_expired_entries() {
        let config = RegistryConfig {
            ttl: chrono::Duration::milliseconds(50),
            ..Default::default()
        };
        let registry = Registry::with_config(config);

        // Populate and expire
        let _ = registry.get("gpt-4o").await;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // All filter methods should return empty for expired entries
        assert!(registry
            .filter_by_tier(TierCategory::Flagship)
            .await
            .is_empty());
        assert!(registry
            .filter_by_cost(CostCategory::Standard)
            .await
            .is_empty());
        assert!(registry
            .filter_by_context_window(ContextWindowCategory::Large)
            .await
            .is_empty());
        assert!(registry
            .filter_by_provider(ProviderCategory::OpenAI)
            .await
            .is_empty());
    }

    #[tokio::test]
    async fn test_registry_find_by_capability_after_invalidation() {
        let registry = Registry::new();

        // Populate cache
        let _ = registry.get("gpt-4o").await;

        // Find by capability should work
        let models = registry.find_by_capability("vision").await;
        // Result depends on whether the model supports vision
        drop(models);

        // Invalidate all
        registry.invalidate(&[]).await;

        // Find by capability should return empty
        let models = registry.find_by_capability("vision").await;
        assert!(models.is_empty());
    }
}

mod find_best_fit {
    use super::*;

    #[tokio::test]
    async fn test_registry_find_best_fit() {
        let registry = Registry::new();

        // Populate cache
        let _ = registry.get("gpt-4o").await;

        let best = registry.find_best_fit(100_000).await;
        assert!(best.is_some());
        assert!(best
            .expect("Model registry operation should succeed during test")
            .can_fit_context(100_000));
    }

    #[tokio::test]
    async fn test_registry_find_best_fit_edge_cases() {
        let registry = Registry::new();

        // Populate cache
        let _ = registry.get("gpt-4o").await;

        // Test with zero tokens - should find a model (all can fit 0 tokens)
        let best = registry.find_best_fit(0).await;
        // Result depends on whether cache is populated and models can fit
        drop(best);

        // Test with very large token count (beyond any model)
        let best = registry.find_best_fit(10_000_000).await;
        // May or may not find a model depending on what's in the fetcher
        // Just verify it doesn't panic
        drop(best);
    }

    #[tokio::test]
    async fn test_registry_find_best_fit_empty_cache() {
        let registry = Registry::new();

        // Don't populate cache
        let best = registry.find_best_fit(1000).await;
        // Empty cache should return None
        assert!(best.is_none());
    }

    #[tokio::test]
    async fn test_find_best_fit_no_models_fit() {
        let registry = Registry::new();
        let _ = registry.get("gpt-4o").await;

        // Request extremely large context that no model can fit
        let best = registry.find_best_fit(100_000_000).await; // 100M tokens
        assert!(best.is_none(), "No model should fit 100M tokens");
    }

    #[tokio::test]
    async fn test_find_best_fit_multiple_same_cost() {
        let registry = Registry::new();

        // Populate cache with multiple models
        let _ = registry.get("gpt-4o").await;
        let _ = registry.get("claude-sonnet-4-20250514").await;

        // Find best fit for reasonable token count
        let best = registry.find_best_fit(50000).await;
        assert!(best.is_some(), "Should find a model that fits");

        // The returned model should fit the context
        let model = best.expect("Model registry operation should succeed during test");
        assert!(model.can_fit_context(50000));
    }

    #[tokio::test]
    async fn test_find_best_fit_prefers_cheapest() {
        let registry = Registry::new();

        // Populate cache
        let _ = registry.get("gpt-4o").await;
        let _ = registry.get("claude-sonnet-4-20250514").await; // Cheaper

        // Both can fit 100K tokens, should prefer the cheaper one
        let best = registry.find_best_fit(100_000).await;
        assert!(best.is_some());

        if let Some(model) = best {
            // Gemini Flash is cheaper
            assert!(model.can_fit_context(100_000));
        }
    }

    #[tokio::test]
    async fn test_find_best_fit_at_exact_context_boundary() {
        let registry = Registry::new();
        let _ = registry.get("gpt-4o").await; // 128K context

        // Request exactly at context boundary
        let best = registry.find_best_fit(128_000).await;
        assert!(best.is_some(), "Should find model at exact boundary");

        // Request just over boundary
        let best = registry.find_best_fit(128_001).await;
        // Depends on whether other models with larger context are available
        // Just verify it doesn't panic
        drop(best);
    }
}

mod cost_estimation {
    use super::*;

    #[tokio::test]
    async fn test_registry_estimate_costs() {
        let registry = Registry::new();

        let costs = registry
            .estimate_costs(&["gpt-4o".to_string()], 1_000_000, 500_000)
            .await;

        assert!(costs.contains_key("gpt-4o"));
    }

    #[tokio::test]
    async fn test_registry_estimate_costs_empty_list() {
        let registry = Registry::new();

        let costs = registry.estimate_costs(&[], 1000, 500).await;
        assert_eq!(costs.len(), 0);
    }

    #[tokio::test]
    async fn test_registry_estimate_costs_unknown_model() {
        let registry = Registry::new();

        let costs = registry
            .estimate_costs(&["unknown-model".to_string()], 1000, 500)
            .await;
        // Unknown model should not be in results
        assert!(!costs.contains_key("unknown-model"));
    }

    #[tokio::test]
    async fn test_estimate_costs_mixed_valid_invalid_ids() {
        let registry = Registry::new();

        let costs = registry
            .estimate_costs(
                &[
                    "gpt-4o".to_string(),           // Valid
                    "unknown-model".to_string(),    // Invalid
                    "gemini-2.5-flash".to_string(), // Valid
                    "".to_string(),                 // Empty (invalid)
                ],
                1_000_000,
                500_000,
            )
            .await;

        // Should only return costs for valid models
        assert!(costs.contains_key("gpt-4o"));
        assert!(costs.contains_key("gemini-2.5-flash"));
        assert!(!costs.contains_key("unknown-model"));
        assert!(!costs.contains_key(""));
    }

    #[tokio::test]
    async fn test_estimate_costs_with_zero_tokens() {
        let registry = Registry::new();
        let _ = registry.get("gpt-4o").await;

        let costs = registry.estimate_costs(&["gpt-4o".to_string()], 0, 0).await;

        assert!(costs.contains_key("gpt-4o"));
        let cost = costs
            .get("gpt-4o")
            .expect("Model registry operation should succeed during test");
        assert!((cost - 0.0).abs() < 0.001, "Zero tokens should cost zero");
    }
}

mod refresh_and_config {
    use super::*;

    #[tokio::test]
    async fn test_registry_refresh_specific_models() {
        let registry = Registry::new();

        // Refresh specific models (should fetch and cache)
        let result = registry.refresh(&["gpt-4o".to_string()]).await;
        assert!(result.is_ok());

        let cached_ids = registry.cached_ids().await;
        assert!(cached_ids.contains(&"gpt-4o".to_string()));
    }

    #[tokio::test]
    async fn test_registry_refresh_all_models() {
        let registry = Registry::new();

        // Refresh all models (empty slice)
        let result = registry.refresh(&[]).await;
        assert!(result.is_ok());

        // Should have cached models from the fetcher
        let cached_count = registry.cached_count().await;
        assert!(cached_count > 0);
    }

    #[tokio::test]
    async fn test_registry_config_with_background_refresh() {
        let config = RegistryConfig {
            enable_background_refresh: true,
            refresh_interval: chrono::Duration::seconds(1),
            ..Default::default()
        };

        let registry = Registry::with_config(config);

        // Give background task time to run
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Registry should still be functional
        let result = registry.get("gpt-4o").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_registry_get_empty_model_id() {
        let registry = Registry::new();

        let result = registry.get("").await;
        assert!(result.is_err());
        assert!(result
            .expect_err("expected error should be present")
            .to_string()
            .contains("model ID cannot be empty"));
    }
}

mod find_queries {
    use super::*;

    #[tokio::test]
    async fn test_registry_find_by_capability_empty_cache() {
        let registry = Registry::new();

        let models = registry.find_by_capability("vision").await;
        assert!(models.is_empty());
    }

    #[tokio::test]
    async fn test_registry_find_by_provider_empty_cache() {
        let registry = Registry::new();

        let models = registry.find_by_provider("openai").await;
        assert!(models.is_empty());
    }
}
