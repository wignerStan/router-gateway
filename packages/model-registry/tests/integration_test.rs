//! Integration tests for the model-registry package.
//!
//! End-to-end tests across registry, categorization, fetcher, and policy modules.

use model_registry::*;

fn config_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("config")
}

// ===== Helper Functions =====

fn test_model(id: &str, provider: &str, price: f64, context: usize) -> ModelInfo {
    ModelInfo {
        id: id.to_string(),
        name: format!("Test {id}"),
        provider: provider.to_string(),
        context_window: context,
        max_output_tokens: 4096,
        input_price_per_million: price,
        output_price_per_million: price * 5.0,
        capabilities: ModelCapabilities {
            streaming: true,
            tools: true,
            vision: true,
            thinking: false,
        },
        rate_limits: RateLimits {
            requests_per_minute: 60,
            tokens_per_minute: 90_000,
        },
        source: DataSource::Static,
    }
}

fn flagship_model() -> ModelInfo {
    test_model("gpt-4-turbo", "openai", 10.0, 128_000)
}

fn standard_model() -> ModelInfo {
    test_model("some-sonnet-model", "test-provider", 3.0, 200_000)
}

fn fast_model() -> ModelInfo {
    test_model("gemini-2.5-flash", "google", 0.075, 1_000_000)
}

fn model_with_capabilities(
    vision: bool,
    tools: bool,
    streaming: bool,
    thinking: bool,
) -> ModelInfo {
    ModelInfo {
        id: "cap-test".to_string(),
        name: "Cap Test".to_string(),
        provider: "test".to_string(),
        context_window: 128_000,
        max_output_tokens: 4096,
        input_price_per_million: 3.0,
        output_price_per_million: 15.0,
        capabilities: ModelCapabilities {
            streaming,
            tools,
            vision,
            thinking,
        },
        rate_limits: RateLimits {
            requests_per_minute: 60,
            tokens_per_minute: 90_000,
        },
        source: DataSource::Static,
    }
}

// ===== 1. Registry Init & get() =====

#[tokio::test]
async fn registry_init_with_static_fetcher_returns_valid_model() {
    let registry = Registry::new();
    let result = registry.get("gpt-4o").await;
    assert!(result.is_ok());
    let model = result.unwrap();
    assert!(model.is_some());
    assert_eq!(model.unwrap().provider, "openai");
}

#[tokio::test]
async fn registry_get_unknown_model_returns_none() {
    let registry = Registry::new();
    let result = registry.get("nonexistent-model-xyz").await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn registry_get_empty_id_returns_error() {
    let registry = Registry::new();
    let result = registry.get("").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty"));
}

#[tokio::test]
async fn registry_get_multiple_fetches_several_models() {
    let registry = Registry::new();
    let ids = vec![
        "gpt-4o".to_string(),
        "gemini-2.5-pro".to_string(),
        "nonexistent".to_string(),
    ];
    let result = registry.get_multiple(&ids).await.unwrap();
    assert_eq!(result.len(), 2);
    assert!(result.contains_key("gpt-4o"));
    assert!(result.contains_key("gemini-2.5-pro"));
}

#[tokio::test]
async fn registry_cache_populates_on_get() {
    let registry = Registry::new();
    assert_eq!(registry.cached_count().await, 0);
    let _ = registry.get("gpt-4o").await;
    assert!(registry.cached_count().await >= 1);
}

#[tokio::test]
async fn registry_invalidate_clears_cache() {
    let registry = Registry::new();
    let _ = registry.get("gpt-4o").await;
    assert!(registry.cached_count().await > 0);
    registry.invalidate(&[]).await;
    assert_eq!(registry.cached_count().await, 0);
}

// ===== 2. ModelCategorization 5-dimension mapping =====

#[test]
fn categorization_returns_all_five_dimensions() {
    let model = test_model("test-model", "openai", 3.0, 200_000);
    let cats = model.get_categories();
    assert!(!cats.capabilities.is_empty());
    assert_ne!(cats.tier, TierCategory::Fast); // $3 is standard
    assert_eq!(cats.cost, CostCategory::Standard);
    assert_eq!(cats.context, ContextWindowCategory::Large);
    assert_eq!(cats.provider, ProviderCategory::OpenAI);
}

#[test]
fn categorization_capability_dimensions() {
    let model = model_with_capabilities(true, true, true, true);
    let caps = model.get_capability_categories();
    assert_eq!(caps.len(), 4);
    assert!(caps.contains(&CapabilityCategory::Vision));
    assert!(caps.contains(&CapabilityCategory::Tools));
    assert!(caps.contains(&CapabilityCategory::Streaming));
    assert!(caps.contains(&CapabilityCategory::Thinking));
}

#[test]
fn categorization_tier_flagship() {
    let model = flagship_model();
    assert_eq!(model.get_tier(), TierCategory::Flagship);
}

#[test]
fn categorization_tier_fast() {
    let model = fast_model();
    assert_eq!(model.get_tier(), TierCategory::Fast);
}

#[test]
fn categorization_cost_boundaries() {
    let ultra = test_model("ultra", "test", 50.0, 100_000);
    assert_eq!(ultra.get_cost_category(), CostCategory::UltraPremium);

    let premium = test_model("premium", "test", 10.0, 100_000);
    assert_eq!(premium.get_cost_category(), CostCategory::Premium);

    let standard = test_model("standard", "test", 1.0, 100_000);
    assert_eq!(standard.get_cost_category(), CostCategory::Standard);

    let economy = test_model("economy", "test", 0.5, 100_000);
    assert_eq!(economy.get_cost_category(), CostCategory::Economy);
}

#[test]
fn categorization_context_boundaries() {
    let small = test_model("s", "test", 1.0, 16_000);
    assert_eq!(small.get_context_category(), ContextWindowCategory::Small);

    let medium = test_model("m", "test", 1.0, 64_000);
    assert_eq!(medium.get_context_category(), ContextWindowCategory::Medium);

    let large = test_model("l", "test", 1.0, 200_000);
    assert_eq!(large.get_context_category(), ContextWindowCategory::Large);

    let ultra = test_model("u", "test", 1.0, 1_000_000);
    assert_eq!(ultra.get_context_category(), ContextWindowCategory::Ultra);
}

#[test]
fn categorization_has_all_any_capability() {
    let model = model_with_capabilities(true, true, false, false);

    assert!(model.has_all_capabilities(&[CapabilityCategory::Vision, CapabilityCategory::Tools,]));
    assert!(
        !model.has_all_capabilities(&[CapabilityCategory::Vision, CapabilityCategory::Thinking,])
    );

    assert!(model.has_any_capability(&[CapabilityCategory::Thinking, CapabilityCategory::Vision]));
    assert!(!model.has_any_capability(&[CapabilityCategory::Thinking]));
}

// ===== 3. ProviderCategory::parse known-provider matching =====

#[test]
fn provider_parse_matches_known_providers() {
    // Core provider names
    assert_eq!(
        ProviderCategory::parse("openai"),
        Some(ProviderCategory::OpenAI)
    );
    assert_eq!(
        ProviderCategory::parse("google"),
        Some(ProviderCategory::Google)
    );
    assert_eq!(
        ProviderCategory::parse("gemini"),
        Some(ProviderCategory::Google)
    );
    // Alias maps to a different category
    assert_eq!(
        ProviderCategory::parse("bedrock"),
        Some(ProviderCategory::Amazon)
    );
    // Another distinct provider
    assert_eq!(ProviderCategory::parse("xai"), Some(ProviderCategory::XAI));
}

#[test]
fn provider_parse_aliases_map_to_correct_category() {
    assert_eq!(
        ProviderCategory::parse("qwen"),
        Some(ProviderCategory::Alibaba)
    );
    assert_eq!(
        ProviderCategory::parse("glm"),
        Some(ProviderCategory::Zhipu)
    );
    assert_eq!(
        ProviderCategory::parse("ernie"),
        Some(ProviderCategory::Baidu)
    );
    assert_eq!(
        ProviderCategory::parse("kimi"),
        Some(ProviderCategory::Moonshot)
    );
    assert_eq!(
        ProviderCategory::parse("doubao"),
        Some(ProviderCategory::ByteDance)
    );
    assert_eq!(
        ProviderCategory::parse("llama"),
        Some(ProviderCategory::MetaLlama)
    );
    assert_eq!(
        ProviderCategory::parse("bedrock"),
        Some(ProviderCategory::Amazon)
    );
    assert_eq!(
        ProviderCategory::parse("vertex"),
        Some(ProviderCategory::VertexAI)
    );
}

#[test]
fn provider_parse_unknown_returns_other() {
    assert_eq!(
        ProviderCategory::parse("futurecorp"),
        Some(ProviderCategory::Other)
    );
    assert_eq!(ProviderCategory::parse(""), Some(ProviderCategory::Other));
}

// ===== 4. Fetcher list_all and fetch_multiple =====

#[tokio::test]
async fn static_fetcher_list_all_returns_static_models() {
    let fetcher = StaticFetcher::new();
    let models = fetcher.list_all().await.unwrap();
    assert!(models.len() >= 6);
    assert!(models.contains_key("gpt-4o"));
    assert!(models.contains_key("gemini-2.5-pro"));
}

#[tokio::test]
async fn static_fetcher_fetch_multiple_filters_unknown() {
    let fetcher = StaticFetcher::new();
    let result = fetcher
        .fetch_multiple(&["gpt-4o".into(), "unknown-model".into()])
        .await
        .unwrap();
    assert_eq!(result.len(), 1);
    assert!(result.contains_key("gpt-4o"));
}

#[tokio::test]
async fn static_fetcher_returns_none_for_unknown() {
    let fetcher = StaticFetcher::new();
    let result = fetcher.fetch("completely-unknown-model").await.unwrap();
    assert!(result.is_none());
}

// ===== 5. Tier-based bandit priors =====

#[test]
fn tier_enum_variants_match_categorization() {
    let flagship = flagship_model();
    let standard = standard_model();
    let fast = fast_model();

    assert!(flagship.is_in_tier(TierCategory::Flagship));
    assert!(standard.is_in_tier(TierCategory::Standard));
    assert!(fast.is_in_tier(TierCategory::Fast));

    assert!(!flagship.is_in_tier(TierCategory::Fast));
    assert!(!fast.is_in_tier(TierCategory::Flagship));
}

#[test]
fn tier_as_str_returns_lowercase() {
    assert_eq!(TierCategory::Flagship.as_str(), "flagship");
    assert_eq!(TierCategory::Standard.as_str(), "standard");
    assert_eq!(TierCategory::Fast.as_str(), "fast");
}

#[test]
fn tier_serializes_correctly() {
    let json = serde_json::to_string(&TierCategory::Flagship).unwrap();
    assert_eq!(json, "\"flagship\"");

    let deserialized: TierCategory = serde_json::from_str("\"fast\"").unwrap();
    assert_eq!(deserialized, TierCategory::Fast);
}

// ===== 6. RoutingPolicy filter end-to-end =====

#[test]
fn routing_policy_filters_by_tier() {
    let policy = RoutingPolicy::new("fast-only", "Fast Only")
        .with_priority(10)
        .with_tier(TierCategory::Fast);

    let registry = PolicyRegistry::new();
    let mut matcher = PolicyMatcher::new(registry);
    matcher.registry_mut().add(policy);

    let context = PolicyContext::default();

    // Fast model should match
    let matches = matcher.evaluate(&fast_model(), &context);
    assert_eq!(matches.len(), 1);

    // Flagship model should not match
    let matches = matcher.evaluate(&flagship_model(), &context);
    assert_eq!(matches.len(), 0);
}

#[test]
fn routing_policy_filters_by_capability() {
    let policy = RoutingPolicy::new("thinking-req", "Thinking Required")
        .with_priority(10)
        .with_capability(CapabilityCategory::Thinking, "require");

    let registry = PolicyRegistry::new();
    let mut matcher = PolicyMatcher::new(registry);
    matcher.registry_mut().add(policy);

    let context = PolicyContext::default();

    let model_with_thinking = model_with_capabilities(true, true, true, true);
    let matches = matcher.evaluate(&model_with_thinking, &context);
    assert_eq!(matches.len(), 1);

    let model_without_thinking = model_with_capabilities(true, true, true, false);
    let matches = matcher.evaluate(&model_without_thinking, &context);
    assert_eq!(matches.len(), 0);
}

#[test]
fn routing_policy_filters_by_provider() {
    let policy = RoutingPolicy::new("openai-only", "OpenAI Only")
        .with_priority(10)
        .with_provider(ProviderCategory::OpenAI);

    let registry = PolicyRegistry::new();
    let mut matcher = PolicyMatcher::new(registry);
    matcher.registry_mut().add(policy);

    let context = PolicyContext::default();

    let matches = matcher.evaluate(&flagship_model(), &context);
    assert_eq!(matches.len(), 1);

    let google_model = fast_model();
    let matches = matcher.evaluate(&google_model, &context);
    assert_eq!(matches.len(), 0);
}

#[test]
fn routing_policy_disabled_does_not_match() {
    let mut policy = RoutingPolicy::new("disabled-policy", "Disabled")
        .with_priority(100)
        .with_tier(TierCategory::Fast);
    policy.enabled = false;

    let registry = PolicyRegistry::new();
    let mut matcher = PolicyMatcher::new(registry);
    matcher.registry_mut().add(policy);

    let context = PolicyContext::default();
    let matches = matcher.evaluate(&fast_model(), &context);
    assert_eq!(matches.len(), 0);
}

// ===== 7. PolicyRegistry with JSON schema validation =====

#[test]
fn policy_registry_from_valid_json_file() {
    let path = config_dir().join("policies.json");
    let registry = PolicyRegistry::from_file(&path);
    assert!(registry.is_ok(), "from_file failed: {:?}", registry.err());
    let registry = registry.unwrap();
    assert!(!registry.all().is_empty());

    // The highest priority policy is "block_ultra_premium" (100) but it's disabled
    let first = &registry.all()[0];
    assert!(first.priority > 0);
    assert_eq!(first.id, "block_ultra_premium");
}

#[test]
fn policy_registry_schema_rejects_invalid_json() {
    let schema = PolicyRegistry::load_schema();

    // Missing "policies" key
    let result = PolicyRegistry::validate_against_schema("{}", &schema);
    assert!(result.is_err());

    // Invalid structure
    let result = PolicyRegistry::validate_against_schema(r#"{"policies": [{"id": "x"}]}"#, &schema);
    assert!(result.is_err());
}

#[test]
fn policy_registry_schema_accepts_valid_config() {
    let schema = PolicyRegistry::load_schema();
    let path = config_dir().join("policies.json");
    let content = std::fs::read_to_string(&path).expect("policies.json should exist");
    let result = PolicyRegistry::validate_against_schema(&content, &schema);
    assert!(
        result.is_ok(),
        "valid config should pass schema: {:?}",
        result.err()
    );
}

#[test]
fn policy_registry_serialization_roundtrip() {
    let mut registry = PolicyRegistry::new();
    registry.add(templates::vision_required());
    registry.add(templates::performance_first());

    let json = registry.to_json().unwrap();
    assert!(json.contains("vision_required"));
    assert!(json.contains("performance_first"));

    let deserialized = PolicyRegistry::from_json(&json).unwrap();
    assert_eq!(deserialized.all().len(), 2);
}

#[test]
fn policy_condition_time_of_day_matching() {
    let mut policy = RoutingPolicy::new("offpeak", "Off-Peak")
        .with_priority(5)
        .with_action("weight")
        .with_weight_factor(0.8);
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TimeOfDay,
        value: "22,23,0,1,2,3,4,5,6".to_string(),
        operator: "in".to_string(),
    });

    let context_midnight = PolicyContext {
        hour_of_day: Some(0),
        ..Default::default()
    };
    assert!(policy.matches(&context_midnight));

    let context_noon = PolicyContext {
        hour_of_day: Some(12),
        ..Default::default()
    };
    assert!(!policy.matches(&context_noon));
}

#[test]
fn policy_condition_token_count_numeric_comparison() {
    let mut policy = RoutingPolicy::new("large-req", "Large Request")
        .with_priority(10)
        .with_action("prefer");
    policy.conditions.push(PolicyCondition {
        condition_type: PolicyConditionType::TokenCount,
        value: "50000".to_string(),
        operator: "gte".to_string(),
    });

    let context_large = PolicyContext {
        token_count: Some(100_000),
        ..Default::default()
    };
    assert!(policy.matches(&context_large));

    let context_small = PolicyContext {
        token_count: Some(10_000),
        ..Default::default()
    };
    assert!(!policy.matches(&context_small));

    // Numeric: 999 < 1000 should be false (not lexicographic)
    let context_999 = PolicyContext {
        token_count: Some(999),
        ..Default::default()
    };
    assert!(!policy.matches(&context_999));
}

// ===== 8. Registry filter helpers =====

#[tokio::test]
async fn registry_filter_by_capability_after_populate() {
    let registry = Registry::new();
    let _ = registry.get("gpt-4o").await;
    let _ = registry.get("gemini-2.5-flash").await;

    let vision_models = registry
        .filter_by_capability(CapabilityCategory::Vision)
        .await;
    assert!(vision_models.len() >= 2);

    let thinking_models = registry
        .filter_by_capability(CapabilityCategory::Thinking)
        .await;
    // None of the static models have thinking except opus
    assert!(thinking_models.len() <= 1);
}

#[tokio::test]
async fn registry_filter_by_tier_after_populate() {
    let registry = Registry::new();
    let _ = registry.get("gpt-4-turbo").await; // flagship
    let _ = registry.get("gemini-2.5-flash").await; // fast

    let flagship = registry.filter_by_tier(TierCategory::Flagship).await;
    assert!(!flagship.is_empty());

    let fast = registry.filter_by_tier(TierCategory::Fast).await;
    assert!(!fast.is_empty());
}

#[tokio::test]
async fn registry_find_best_fit_picks_cheapest() {
    let registry = Registry::new();
    let _ = registry.get("gpt-4o").await;
    let _ = registry.get("gemini-2.5-flash").await;
    let _ = registry.get("gemini-2.5-pro").await;

    // Request that fits all models
    let best = registry.find_best_fit(100_000).await;
    assert!(best.is_some());
    // gemini-2.5-flash is the cheapest ($0.075/M input)
    assert_eq!(best.unwrap().id, "gemini-2.5-flash");
}
