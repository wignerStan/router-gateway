use crate::info::ModelInfo;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Functional capability of a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityCategory {
    /// Image/vision input support.
    Vision,
    /// Function/tool calling support.
    Tools,
    /// Streaming response support.
    Streaming,
    /// Extended thinking/reasoning support.
    Thinking,
}

impl CapabilityCategory {
    /// Returns the `snake_case` string representation.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        match self {
            Self::Vision => "vision",
            Self::Tools => "tools",
            Self::Streaming => "streaming",
            Self::Thinking => "thinking",
        }
    }
}

/// Quality or performance tier for model classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierCategory {
    /// Flagship models (Opus, GPT-4).
    Flagship,
    /// Standard quality (Sonnet, GPT-4o).
    Standard,
    /// Economy/fast (Haiku, GPT-4o-mini, Flash).
    Fast,
}

impl TierCategory {
    /// Returns the `snake_case` string representation.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        match self {
            Self::Flagship => "flagship",
            Self::Standard => "standard",
            Self::Fast => "fast",
        }
    }
}

/// Pricing band based on input price per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostCategory {
    /// $50+/1M tokens.
    UltraPremium,
    /// $10-50/1M tokens.
    Premium,
    /// $1-10/1M tokens.
    Standard,
    /// <$1/1M tokens.
    Economy,
}

impl CostCategory {
    /// Returns the `snake_case` string representation.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        match self {
            Self::UltraPremium => "ultra_premium",
            Self::Premium => "premium",
            Self::Standard => "standard",
            Self::Economy => "economy",
        }
    }
}

/// Context window size band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextWindowCategory {
    /// <32K tokens.
    Small,
    /// 32K-128K tokens.
    Medium,
    /// 128K-500K tokens.
    Large,
    /// 500K+ tokens.
    Ultra,
}

impl ContextWindowCategory {
    /// Returns the `snake_case` string representation.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
            Self::Ultra => "ultra",
        }
    }
}

/// Model vendor for provider-based routing and filtering.
///
/// Extended to support providers from models.dev.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCategory {
    // Major Cloud Providers
    /// Anthropic (Claude).
    Anthropic,
    /// `OpenAI` (GPT).
    #[serde(rename = "openai")]
    OpenAI,
    /// Google (Gemini).
    Google,

    // Emerging AI Companies
    /// xAI (Grok).
    #[serde(rename = "xai")]
    XAI,
    /// `DeepSeek`.
    #[serde(rename = "deepseek")]
    DeepSeek,
    /// Mistral AI.
    Mistral,
    /// Cohere.
    Cohere,
    /// Perplexity AI.
    Perplexity,

    // Chinese Providers
    /// Alibaba (Qwen/Tongyi).
    Alibaba,
    /// Zhipu AI (GLM/ChatGLM).
    Zhipu,
    /// Baidu (ERNIE/Wenxin).
    Baidu,
    /// Moonshot AI (Kimi).
    Moonshot,
    /// `ByteDance` (Doubao).
    #[serde(rename = "bytedance")]
    ByteDance,

    // Open Source / Community
    /// Meta (Llama).
    Meta,
    /// Meta Llama (alias).
    #[serde(rename = "meta-llama")]
    MetaLlama,
    /// Databricks.
    Databricks,
    /// Stability AI.
    Stability,

    // Cloud Platforms
    /// Amazon (AWS Bedrock).
    Amazon,
    /// Azure `OpenAI`.
    Azure,
    /// Google Vertex AI.
    #[serde(rename = "vertexai")]
    VertexAI,

    // Other
    /// Unrecognized or unspecified provider.
    Other,
}

impl ProviderCategory {
    /// Returns the `snake_case` string representation.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAI => "openai",
            Self::Google => "google",
            Self::XAI => "xai",
            Self::DeepSeek => "deepseek",
            Self::Mistral => "mistral",
            Self::Cohere => "cohere",
            Self::Perplexity => "perplexity",
            Self::Alibaba => "alibaba",
            Self::Zhipu => "zhipu",
            Self::Baidu => "baidu",
            Self::Moonshot => "moonshot",
            Self::ByteDance => "bytedance",
            Self::Meta => "meta",
            Self::MetaLlama => "meta-llama",
            Self::Databricks => "databricks",
            Self::Stability => "stability",
            Self::Amazon => "amazon",
            Self::Azure => "azure",
            Self::VertexAI => "vertexai",
            Self::Other => "other",
        }
    }

    /// Parses a provider string (case-insensitive, alias-aware).
    ///
    /// Unknown providers map to [`Other`].
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "anthropic" => Some(Self::Anthropic),
            "openai" => Some(Self::OpenAI),
            "google" | "gemini" => Some(Self::Google),
            "xai" | "x-ai" | "grok" => Some(Self::XAI),
            "deepseek" => Some(Self::DeepSeek),
            "mistral" | "mistralai" => Some(Self::Mistral),
            "cohere" => Some(Self::Cohere),
            "perplexity" => Some(Self::Perplexity),
            "alibaba" | "qwen" | "tongyi" => Some(Self::Alibaba),
            "zhipu" | "glm" | "chatglm" => Some(Self::Zhipu),
            "baidu" | "ernie" | "wenxin" => Some(Self::Baidu),
            "moonshot" | "kimi" => Some(Self::Moonshot),
            "bytedance" | "doubao" => Some(Self::ByteDance),
            "meta" => Some(Self::Meta),
            "meta-llama" | "llama" => Some(Self::MetaLlama),
            "databricks" => Some(Self::Databricks),
            "stability" => Some(Self::Stability),
            "amazon" | "bedrock" | "aws" => Some(Self::Amazon),
            "azure" => Some(Self::Azure),
            "vertexai" | "vertex" => Some(Self::VertexAI),
            _ => Some(Self::Other),
        }
    }

    /// Returns `true` for providers headquartered in China.
    #[must_use]
    pub const fn is_chinese_provider(&self) -> bool {
        matches!(
            self,
            Self::Alibaba
                | Self::Zhipu
                | Self::Baidu
                | Self::Moonshot
                | Self::ByteDance
                | Self::DeepSeek
        )
    }

    /// Returns `true` for cloud platforms (as opposed to direct model providers).
    #[must_use]
    pub const fn is_cloud_platform(&self) -> bool {
        matches!(self, Self::Amazon | Self::Azure | Self::VertexAI)
    }

    /// Human-readable display name for UI rendering.
    #[must_use]
    pub const fn display_name(&self) -> &str {
        match self {
            Self::Anthropic => "Anthropic",
            Self::OpenAI => "OpenAI",
            Self::Google => "Google",
            Self::XAI => "xAI",
            Self::DeepSeek => "DeepSeek",
            Self::Mistral => "Mistral AI",
            Self::Cohere => "Cohere",
            Self::Perplexity => "Perplexity",
            Self::Alibaba => "Alibaba (Qwen)",
            Self::Zhipu => "Zhipu AI (GLM)",
            Self::Baidu => "Baidu (ERNIE)",
            Self::Moonshot => "Moonshot (Kimi)",
            Self::ByteDance => "ByteDance (Doubao)",
            Self::Meta => "Meta",
            Self::MetaLlama => "Meta (Llama)",
            Self::Databricks => "Databricks",
            Self::Stability => "Stability AI",
            Self::Amazon => "Amazon Bedrock",
            Self::Azure => "Azure OpenAI",
            Self::VertexAI => "Google Vertex AI",
            Self::Other => "Other",
        }
    }
}

/// Aggregated category set for a single model across all classification dimensions.
#[derive(Debug, Clone)]
pub struct ModelCategorySet {
    /// Capabilities the model supports.
    pub capabilities: Vec<CapabilityCategory>,
    /// Quality/performance tier.
    pub tier: TierCategory,
    /// Pricing band.
    pub cost: CostCategory,
    /// Context window size band.
    pub context: ContextWindowCategory,
    /// Provider vendor.
    pub provider: ProviderCategory,
}

/// Extension trait for multi-dimension classification on [`ModelInfo`].
pub trait ModelCategorization {
    /// Returns all categories for the model.
    fn get_categories(&self) -> ModelCategorySet;
    /// Returns the model's capability categories.
    fn get_capability_categories(&self) -> Vec<CapabilityCategory>;
    /// Returns the model's quality tier.
    fn get_tier(&self) -> TierCategory;
    /// Returns the model's pricing band.
    fn get_cost_category(&self) -> CostCategory;
    /// Returns the model's context window band.
    fn get_context_category(&self) -> ContextWindowCategory;
    /// Returns the model's provider category.
    fn get_provider_category(&self) -> ProviderCategory;
    /// Returns `true` if the model has all specified capabilities.
    fn has_all_capabilities(&self, caps: &[CapabilityCategory]) -> bool;
    /// Returns `true` if the model has any of the specified capabilities.
    fn has_any_capability(&self, caps: &[CapabilityCategory]) -> bool;
    /// Returns `true` if the model belongs to the given tier.
    fn is_in_tier(&self, tier: TierCategory) -> bool;
    /// Returns `true` if the model is in the given cost band.
    fn is_in_cost_range(&self, cost: CostCategory) -> bool;
    /// Returns `true` if the model is in the given context window band.
    fn is_in_context_range(&self, context: ContextWindowCategory) -> bool;
    /// Returns `true` if the model is from the given provider.
    fn is_from_provider(&self, provider: ProviderCategory) -> bool;
}

impl ModelCategorization for ModelInfo {
    fn get_categories(&self) -> ModelCategorySet {
        ModelCategorySet {
            capabilities: self.get_capability_categories(),
            tier: self.get_tier(),
            cost: self.get_cost_category(),
            context: self.get_context_category(),
            provider: self.get_provider_category(),
        }
    }

    fn get_capability_categories(&self) -> Vec<CapabilityCategory> {
        let mut caps = Vec::new();
        if self.capabilities.vision {
            caps.push(CapabilityCategory::Vision);
        }
        if self.capabilities.tools {
            caps.push(CapabilityCategory::Tools);
        }
        if self.capabilities.streaming {
            caps.push(CapabilityCategory::Streaming);
        }
        if self.capabilities.thinking {
            caps.push(CapabilityCategory::Thinking);
        }
        caps
    }

    fn get_tier(&self) -> TierCategory {
        // Flagship models: highest quality, highest cost
        let flagship_models = HashSet::from([
            "claude-opus-4",
            "claude-opus-4-20250514",
            "gpt-4",
            "gpt-4-turbo",
            "gpt-4-0314",
            "gemini-2.5-pro",
        ]);

        if flagship_models.contains(self.id.as_str()) || self.input_price_per_million >= 15.0 {
            return TierCategory::Flagship;
        }

        // Fast models: lowest cost, highest speed
        let fast_models = HashSet::from([
            "claude-haiku-4",
            "claude-haiku-4-20250514",
            "gpt-4o-mini",
            "gpt-4o-mini-2024-07-18",
            "gemini-2.0-flash-exp",
            "gemini-2.5-flash",
            "gemini-2.5-flash-exp",
            "gemini-1.5-flash",
            "gemini-1.5-flash-8b",
            "gemini-1.5-flash-exp",
        ]);

        if fast_models.contains(self.id.as_str()) || self.input_price_per_million <= 1.0 {
            return TierCategory::Fast;
        }

        // Default to standard tier
        TierCategory::Standard
    }

    fn get_cost_category(&self) -> CostCategory {
        let price = self.input_price_per_million;
        match price {
            p if p >= 50.0 => CostCategory::UltraPremium,
            p if p >= 10.0 => CostCategory::Premium,
            p if p >= 1.0 => CostCategory::Standard,
            _ => CostCategory::Economy,
        }
    }

    fn get_context_category(&self) -> ContextWindowCategory {
        let window = self.context_window;
        match window {
            w if w >= 500_000 => ContextWindowCategory::Ultra,
            w if w >= 128_000 => ContextWindowCategory::Large,
            w if w >= 32_000 => ContextWindowCategory::Medium,
            _ => ContextWindowCategory::Small,
        }
    }

    fn get_provider_category(&self) -> ProviderCategory {
        ProviderCategory::parse(self.provider.as_str()).unwrap_or(ProviderCategory::Other)
    }

    fn has_all_capabilities(&self, caps: &[CapabilityCategory]) -> bool {
        caps.iter().all(|cap| match cap {
            CapabilityCategory::Vision => self.capabilities.vision,
            CapabilityCategory::Tools => self.capabilities.tools,
            CapabilityCategory::Streaming => self.capabilities.streaming,
            CapabilityCategory::Thinking => self.capabilities.thinking,
        })
    }

    fn has_any_capability(&self, caps: &[CapabilityCategory]) -> bool {
        caps.iter().any(|cap| match cap {
            CapabilityCategory::Vision => self.capabilities.vision,
            CapabilityCategory::Tools => self.capabilities.tools,
            CapabilityCategory::Streaming => self.capabilities.streaming,
            CapabilityCategory::Thinking => self.capabilities.thinking,
        })
    }

    fn is_in_tier(&self, tier: TierCategory) -> bool {
        self.get_tier() == tier
    }

    fn is_in_cost_range(&self, cost: CostCategory) -> bool {
        self.get_cost_category() == cost
    }

    fn is_in_context_range(&self, context: ContextWindowCategory) -> bool {
        self.get_context_category() == context
    }

    fn is_from_provider(&self, provider: ProviderCategory) -> bool {
        self.get_provider_category() == provider
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::info::{DataSource, ModelCapabilities, RateLimits};

    fn create_test_model(id: &str, provider: &str, price: f64, context: usize) -> ModelInfo {
        ModelInfo {
            id: id.to_string(),
            name: "Test Model".to_string(),
            provider: provider.to_string(),
            context_window: context,
            max_output_tokens: 4096,
            input_price_per_million: price,
            output_price_per_million: price * 2.0,
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

    #[test]
    fn test_tier_categorization() {
        // Flagship model
        let flagship = create_test_model("claude-opus-4-20250514", "anthropic", 15.0, 200_000);
        assert_eq!(flagship.get_tier(), TierCategory::Flagship);

        // Standard model
        let standard = create_test_model("claude-sonnet-4-20250514", "anthropic", 3.0, 200_000);
        assert_eq!(standard.get_tier(), TierCategory::Standard);

        // Fast model
        let fast = create_test_model("gemini-2.5-flash", "google", 0.075, 1_000_000);
        assert_eq!(fast.get_tier(), TierCategory::Fast);
    }

    #[test]
    fn test_cost_categorization() {
        let ultra_premium = create_test_model("test", "test", 60.0, 100_000);
        assert_eq!(
            ultra_premium.get_cost_category(),
            CostCategory::UltraPremium
        );

        let premium = create_test_model("test", "test", 15.0, 100_000);
        assert_eq!(premium.get_cost_category(), CostCategory::Premium);

        let standard = create_test_model("test", "test", 3.0, 100_000);
        assert_eq!(standard.get_cost_category(), CostCategory::Standard);

        let economy = create_test_model("test", "test", 0.5, 100_000);
        assert_eq!(economy.get_cost_category(), CostCategory::Economy);
    }

    #[test]
    fn test_context_categorization() {
        let ultra = create_test_model("test", "test", 1.0, 1_000_000);
        assert_eq!(ultra.get_context_category(), ContextWindowCategory::Ultra);

        let large = create_test_model("test", "test", 1.0, 200_000);
        assert_eq!(large.get_context_category(), ContextWindowCategory::Large);

        let medium = create_test_model("test", "test", 1.0, 100_000);
        assert_eq!(medium.get_context_category(), ContextWindowCategory::Medium);

        let small = create_test_model("test", "test", 1.0, 16000);
        assert_eq!(small.get_context_category(), ContextWindowCategory::Small);
    }

    #[test]
    fn test_capability_checks() {
        let model = create_test_model("test", "test", 1.0, 100_000);

        assert!(model.has_all_capabilities(&[
            CapabilityCategory::Streaming,
            CapabilityCategory::Tools,
            CapabilityCategory::Vision,
        ]));

        assert!(!model
            .has_all_capabilities(&[CapabilityCategory::Streaming, CapabilityCategory::Thinking,]));

        assert!(
            model.has_any_capability(&[CapabilityCategory::Vision, CapabilityCategory::Thinking,])
        );

        assert!(!model.has_any_capability(&[CapabilityCategory::Thinking]));
    }

    // ========================================
    // ProviderCategory::parse tests
    // ========================================

    #[test]
    fn test_provider_parse_standard_names() {
        // Standard lowercase names
        assert_eq!(
            ProviderCategory::parse("anthropic"),
            Some(ProviderCategory::Anthropic)
        );
        assert_eq!(
            ProviderCategory::parse("openai"),
            Some(ProviderCategory::OpenAI)
        );
        assert_eq!(
            ProviderCategory::parse("google"),
            Some(ProviderCategory::Google)
        );
        assert_eq!(ProviderCategory::parse("xai"), Some(ProviderCategory::XAI));
        assert_eq!(
            ProviderCategory::parse("deepseek"),
            Some(ProviderCategory::DeepSeek)
        );
        assert_eq!(
            ProviderCategory::parse("mistral"),
            Some(ProviderCategory::Mistral)
        );
        assert_eq!(
            ProviderCategory::parse("cohere"),
            Some(ProviderCategory::Cohere)
        );
        assert_eq!(
            ProviderCategory::parse("perplexity"),
            Some(ProviderCategory::Perplexity)
        );
    }

    #[test]
    fn test_provider_parse_aliases() {
        // Google aliases
        assert_eq!(
            ProviderCategory::parse("gemini"),
            Some(ProviderCategory::Google)
        );

        // xAI aliases
        assert_eq!(ProviderCategory::parse("x-ai"), Some(ProviderCategory::XAI));
        assert_eq!(ProviderCategory::parse("grok"), Some(ProviderCategory::XAI));

        // Mistral alias
        assert_eq!(
            ProviderCategory::parse("mistralai"),
            Some(ProviderCategory::Mistral)
        );

        // Alibaba aliases
        assert_eq!(
            ProviderCategory::parse("qwen"),
            Some(ProviderCategory::Alibaba)
        );
        assert_eq!(
            ProviderCategory::parse("tongyi"),
            Some(ProviderCategory::Alibaba)
        );

        // Zhipu aliases
        assert_eq!(
            ProviderCategory::parse("glm"),
            Some(ProviderCategory::Zhipu)
        );
        assert_eq!(
            ProviderCategory::parse("chatglm"),
            Some(ProviderCategory::Zhipu)
        );

        // Baidu aliases
        assert_eq!(
            ProviderCategory::parse("ernie"),
            Some(ProviderCategory::Baidu)
        );
        assert_eq!(
            ProviderCategory::parse("wenxin"),
            Some(ProviderCategory::Baidu)
        );

        // Moonshot alias
        assert_eq!(
            ProviderCategory::parse("kimi"),
            Some(ProviderCategory::Moonshot)
        );

        // ByteDance alias
        assert_eq!(
            ProviderCategory::parse("doubao"),
            Some(ProviderCategory::ByteDance)
        );

        // Meta Llama aliases
        assert_eq!(
            ProviderCategory::parse("meta-llama"),
            Some(ProviderCategory::MetaLlama)
        );
        assert_eq!(
            ProviderCategory::parse("llama"),
            Some(ProviderCategory::MetaLlama)
        );

        // Amazon aliases
        assert_eq!(
            ProviderCategory::parse("bedrock"),
            Some(ProviderCategory::Amazon)
        );
        assert_eq!(
            ProviderCategory::parse("aws"),
            Some(ProviderCategory::Amazon)
        );

        // Vertex AI aliases
        assert_eq!(
            ProviderCategory::parse("vertex"),
            Some(ProviderCategory::VertexAI)
        );
    }

    #[test]
    fn test_provider_parse_case_variations() {
        // Parse is case-insensitive (converts to lowercase internally)
        assert_eq!(
            ProviderCategory::parse("ANTHROPIC"),
            Some(ProviderCategory::Anthropic)
        );
        assert_eq!(
            ProviderCategory::parse("Anthropic"),
            Some(ProviderCategory::Anthropic)
        );
        assert_eq!(
            ProviderCategory::parse("OpenAI"),
            Some(ProviderCategory::OpenAI)
        );
        assert_eq!(
            ProviderCategory::parse("OPENAI"),
            Some(ProviderCategory::OpenAI)
        );
        assert_eq!(
            ProviderCategory::parse("Google"),
            Some(ProviderCategory::Google)
        );
        assert_eq!(
            ProviderCategory::parse("GOOGLE"),
            Some(ProviderCategory::Google)
        );
        assert_eq!(
            ProviderCategory::parse("DeepSeek"),
            Some(ProviderCategory::DeepSeek)
        );
        assert_eq!(
            ProviderCategory::parse("DEEPSEEK"),
            Some(ProviderCategory::DeepSeek)
        );
        assert_eq!(
            ProviderCategory::parse("Qwen"),
            Some(ProviderCategory::Alibaba)
        );
        assert_eq!(
            ProviderCategory::parse("QWEN"),
            Some(ProviderCategory::Alibaba)
        );
        assert_eq!(ProviderCategory::parse("Grok"), Some(ProviderCategory::XAI));
        assert_eq!(ProviderCategory::parse("GROK"), Some(ProviderCategory::XAI));
    }

    #[test]
    fn test_provider_parse_unknown_returns_other() {
        // Unknown providers return Other, not None
        assert_eq!(
            ProviderCategory::parse("unknown-provider"),
            Some(ProviderCategory::Other)
        );
        assert_eq!(
            ProviderCategory::parse("random"),
            Some(ProviderCategory::Other)
        );
        assert_eq!(
            ProviderCategory::parse("future-ai"),
            Some(ProviderCategory::Other)
        );
        assert_eq!(ProviderCategory::parse(""), Some(ProviderCategory::Other));
    }

    // ========================================
    // is_chinese_provider tests
    // ========================================

    #[test]
    fn test_is_chinese_provider_all_chinese() {
        // All Chinese providers
        assert!(ProviderCategory::Alibaba.is_chinese_provider());
        assert!(ProviderCategory::Zhipu.is_chinese_provider());
        assert!(ProviderCategory::Baidu.is_chinese_provider());
        assert!(ProviderCategory::Moonshot.is_chinese_provider());
        assert!(ProviderCategory::ByteDance.is_chinese_provider());
        assert!(ProviderCategory::DeepSeek.is_chinese_provider());
    }

    #[test]
    fn test_is_chinese_provider_non_chinese() {
        // Non-Chinese providers
        assert!(!ProviderCategory::Anthropic.is_chinese_provider());
        assert!(!ProviderCategory::OpenAI.is_chinese_provider());
        assert!(!ProviderCategory::Google.is_chinese_provider());
        assert!(!ProviderCategory::XAI.is_chinese_provider());
        assert!(!ProviderCategory::Mistral.is_chinese_provider());
        assert!(!ProviderCategory::Cohere.is_chinese_provider());
        assert!(!ProviderCategory::Perplexity.is_chinese_provider());
        assert!(!ProviderCategory::Meta.is_chinese_provider());
        assert!(!ProviderCategory::MetaLlama.is_chinese_provider());
        assert!(!ProviderCategory::Databricks.is_chinese_provider());
        assert!(!ProviderCategory::Stability.is_chinese_provider());
        assert!(!ProviderCategory::Amazon.is_chinese_provider());
        assert!(!ProviderCategory::Azure.is_chinese_provider());
        assert!(!ProviderCategory::VertexAI.is_chinese_provider());
        assert!(!ProviderCategory::Other.is_chinese_provider());
    }

    // ========================================
    // is_cloud_platform tests
    // ========================================

    #[test]
    fn test_is_cloud_platform_all_cloud_platforms() {
        // All cloud platforms
        assert!(ProviderCategory::Amazon.is_cloud_platform());
        assert!(ProviderCategory::Azure.is_cloud_platform());
        assert!(ProviderCategory::VertexAI.is_cloud_platform());
    }

    #[test]
    fn test_is_cloud_platform_non_cloud() {
        // Direct model providers (not cloud platforms)
        assert!(!ProviderCategory::Anthropic.is_cloud_platform());
        assert!(!ProviderCategory::OpenAI.is_cloud_platform());
        assert!(!ProviderCategory::Google.is_cloud_platform());
        assert!(!ProviderCategory::XAI.is_cloud_platform());
        assert!(!ProviderCategory::DeepSeek.is_cloud_platform());
        assert!(!ProviderCategory::Mistral.is_cloud_platform());
        assert!(!ProviderCategory::Cohere.is_cloud_platform());
        assert!(!ProviderCategory::Perplexity.is_cloud_platform());
        assert!(!ProviderCategory::Alibaba.is_cloud_platform());
        assert!(!ProviderCategory::Zhipu.is_cloud_platform());
        assert!(!ProviderCategory::Baidu.is_cloud_platform());
        assert!(!ProviderCategory::Moonshot.is_cloud_platform());
        assert!(!ProviderCategory::ByteDance.is_cloud_platform());
        assert!(!ProviderCategory::Meta.is_cloud_platform());
        assert!(!ProviderCategory::MetaLlama.is_cloud_platform());
        assert!(!ProviderCategory::Databricks.is_cloud_platform());
        assert!(!ProviderCategory::Stability.is_cloud_platform());
        assert!(!ProviderCategory::Other.is_cloud_platform());
    }

    // ========================================
    // ProviderCategory display_name tests
    // ========================================

    #[test]
    fn test_provider_display_names() {
        assert_eq!(ProviderCategory::Anthropic.display_name(), "Anthropic");
        assert_eq!(ProviderCategory::OpenAI.display_name(), "OpenAI");
        assert_eq!(ProviderCategory::Google.display_name(), "Google");
        assert_eq!(ProviderCategory::XAI.display_name(), "xAI");
        assert_eq!(ProviderCategory::DeepSeek.display_name(), "DeepSeek");
        assert_eq!(ProviderCategory::Mistral.display_name(), "Mistral AI");
        assert_eq!(ProviderCategory::Cohere.display_name(), "Cohere");
        assert_eq!(ProviderCategory::Perplexity.display_name(), "Perplexity");
        assert_eq!(ProviderCategory::Alibaba.display_name(), "Alibaba (Qwen)");
        assert_eq!(ProviderCategory::Zhipu.display_name(), "Zhipu AI (GLM)");
        assert_eq!(ProviderCategory::Baidu.display_name(), "Baidu (ERNIE)");
        assert_eq!(ProviderCategory::Moonshot.display_name(), "Moonshot (Kimi)");
        assert_eq!(
            ProviderCategory::ByteDance.display_name(),
            "ByteDance (Doubao)"
        );
        assert_eq!(ProviderCategory::Meta.display_name(), "Meta");
        assert_eq!(ProviderCategory::MetaLlama.display_name(), "Meta (Llama)");
        assert_eq!(ProviderCategory::Databricks.display_name(), "Databricks");
        assert_eq!(ProviderCategory::Stability.display_name(), "Stability AI");
        assert_eq!(ProviderCategory::Amazon.display_name(), "Amazon Bedrock");
        assert_eq!(ProviderCategory::Azure.display_name(), "Azure OpenAI");
        assert_eq!(
            ProviderCategory::VertexAI.display_name(),
            "Google Vertex AI"
        );
        assert_eq!(ProviderCategory::Other.display_name(), "Other");
    }

    // ========================================
    // ProviderCategory as_str tests
    // ========================================

    #[test]
    fn test_provider_as_str() {
        assert_eq!(ProviderCategory::Anthropic.as_str(), "anthropic");
        assert_eq!(ProviderCategory::OpenAI.as_str(), "openai");
        assert_eq!(ProviderCategory::Google.as_str(), "google");
        assert_eq!(ProviderCategory::XAI.as_str(), "xai");
        assert_eq!(ProviderCategory::DeepSeek.as_str(), "deepseek");
        assert_eq!(ProviderCategory::Mistral.as_str(), "mistral");
        assert_eq!(ProviderCategory::Cohere.as_str(), "cohere");
        assert_eq!(ProviderCategory::Perplexity.as_str(), "perplexity");
        assert_eq!(ProviderCategory::Alibaba.as_str(), "alibaba");
        assert_eq!(ProviderCategory::Zhipu.as_str(), "zhipu");
        assert_eq!(ProviderCategory::Baidu.as_str(), "baidu");
        assert_eq!(ProviderCategory::Moonshot.as_str(), "moonshot");
        assert_eq!(ProviderCategory::ByteDance.as_str(), "bytedance");
        assert_eq!(ProviderCategory::Meta.as_str(), "meta");
        assert_eq!(ProviderCategory::MetaLlama.as_str(), "meta-llama");
        assert_eq!(ProviderCategory::Databricks.as_str(), "databricks");
        assert_eq!(ProviderCategory::Stability.as_str(), "stability");
        assert_eq!(ProviderCategory::Amazon.as_str(), "amazon");
        assert_eq!(ProviderCategory::Azure.as_str(), "azure");
        assert_eq!(ProviderCategory::VertexAI.as_str(), "vertexai");
        assert_eq!(ProviderCategory::Other.as_str(), "other");
    }
}
