use crate::info::ModelInfo;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// CapabilityCategory represents a functional capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityCategory {
    Vision,
    Tools,
    Streaming,
    Thinking,
}

impl CapabilityCategory {
    pub fn as_str(&self) -> &str {
        match self {
            CapabilityCategory::Vision => "vision",
            CapabilityCategory::Tools => "tools",
            CapabilityCategory::Streaming => "streaming",
            CapabilityCategory::Thinking => "thinking",
        }
    }
}

/// TierCategory represents quality/performance tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierCategory {
    /// Flagship models (Opus, GPT-4)
    Flagship,
    /// Standard quality (Sonnet, GPT-4o)
    Standard,
    /// Economy/fast (Haiku, GPT-4o-mini, Flash)
    Fast,
}

impl TierCategory {
    pub fn as_str(&self) -> &str {
        match self {
            TierCategory::Flagship => "flagship",
            TierCategory::Standard => "standard",
            TierCategory::Fast => "fast",
        }
    }
}

/// CostCategory represents pricing band based on input price per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostCategory {
    /// $50+/1M tokens
    UltraPremium,
    /// $10-50/1M tokens
    Premium,
    /// $1-10/1M tokens
    Standard,
    /// <$1/1M tokens
    Economy,
}

impl CostCategory {
    pub fn as_str(&self) -> &str {
        match self {
            CostCategory::UltraPremium => "ultra_premium",
            CostCategory::Premium => "premium",
            CostCategory::Standard => "standard",
            CostCategory::Economy => "economy",
        }
    }
}

/// ContextWindowCategory represents context size band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextWindowCategory {
    /// <32K tokens
    Small,
    /// 32K-128K tokens
    Medium,
    /// 128K-500K tokens
    Large,
    /// 500K+ tokens
    Ultra,
}

impl ContextWindowCategory {
    pub fn as_str(&self) -> &str {
        match self {
            ContextWindowCategory::Small => "small",
            ContextWindowCategory::Medium => "medium",
            ContextWindowCategory::Large => "large",
            ContextWindowCategory::Ultra => "ultra",
        }
    }
}

/// ProviderCategory represents model vendor.
/// Extended to support providers from models.dev
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCategory {
    // Major Cloud Providers
    Anthropic,
    #[serde(rename = "openai")]
    OpenAI,
    Google,

    // Emerging AI Companies
    #[serde(rename = "xai")]
    XAI, // xAI Grok
    #[serde(rename = "deepseek")]
    DeepSeek, // DeepSeek
    Mistral,    // Mistral AI
    Cohere,     // Cohere
    Perplexity, // Perplexity AI

    // Chinese Providers
    Alibaba,  // Qwen/Tongyi
    Zhipu,    // GLM/ChatGLM
    Baidu,    // ERNIE/Wenxin
    Moonshot, // Kimi
    #[serde(rename = "bytedance")]
    ByteDance, // Doubao

    // Open Source / Community
    Meta, // Llama
    #[serde(rename = "meta-llama")]
    MetaLlama, // Alias for Meta
    Databricks, // Dolly
    Stability, // Stable LM

    // Cloud Platforms
    Amazon, // AWS Bedrock
    Azure,  // Azure OpenAI
    #[serde(rename = "vertexai")]
    VertexAI, // GCP Vertex AI (alias for Google)

    // Other
    Other,
}

impl ProviderCategory {
    pub fn as_str(&self) -> &str {
        match self {
            ProviderCategory::Anthropic => "anthropic",
            ProviderCategory::OpenAI => "openai",
            ProviderCategory::Google => "google",
            ProviderCategory::XAI => "xai",
            ProviderCategory::DeepSeek => "deepseek",
            ProviderCategory::Mistral => "mistral",
            ProviderCategory::Cohere => "cohere",
            ProviderCategory::Perplexity => "perplexity",
            ProviderCategory::Alibaba => "alibaba",
            ProviderCategory::Zhipu => "zhipu",
            ProviderCategory::Baidu => "baidu",
            ProviderCategory::Moonshot => "moonshot",
            ProviderCategory::ByteDance => "bytedance",
            ProviderCategory::Meta => "meta",
            ProviderCategory::MetaLlama => "meta-llama",
            ProviderCategory::Databricks => "databricks",
            ProviderCategory::Stability => "stability",
            ProviderCategory::Amazon => "amazon",
            ProviderCategory::Azure => "azure",
            ProviderCategory::VertexAI => "vertexai",
            ProviderCategory::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "anthropic" => Some(ProviderCategory::Anthropic),
            "openai" => Some(ProviderCategory::OpenAI),
            "google" | "gemini" => Some(ProviderCategory::Google),
            "xai" | "x-ai" | "grok" => Some(ProviderCategory::XAI),
            "deepseek" => Some(ProviderCategory::DeepSeek),
            "mistral" | "mistralai" => Some(ProviderCategory::Mistral),
            "cohere" => Some(ProviderCategory::Cohere),
            "perplexity" => Some(ProviderCategory::Perplexity),
            "alibaba" | "qwen" | "tongyi" => Some(ProviderCategory::Alibaba),
            "zhipu" | "glm" | "chatglm" => Some(ProviderCategory::Zhipu),
            "baidu" | "ernie" | "wenxin" => Some(ProviderCategory::Baidu),
            "moonshot" | "kimi" => Some(ProviderCategory::Moonshot),
            "bytedance" | "doubao" => Some(ProviderCategory::ByteDance),
            "meta" => Some(ProviderCategory::Meta),
            "meta-llama" | "llama" => Some(ProviderCategory::MetaLlama),
            "databricks" => Some(ProviderCategory::Databricks),
            "stability" => Some(ProviderCategory::Stability),
            "amazon" | "bedrock" | "aws" => Some(ProviderCategory::Amazon),
            "azure" => Some(ProviderCategory::Azure),
            "vertexai" | "vertex" => Some(ProviderCategory::VertexAI),
            _ => Some(ProviderCategory::Other),
        }
    }

    /// Check if this is a Chinese provider
    pub fn is_chinese_provider(&self) -> bool {
        matches!(
            self,
            ProviderCategory::Alibaba
                | ProviderCategory::Zhipu
                | ProviderCategory::Baidu
                | ProviderCategory::Moonshot
                | ProviderCategory::ByteDance
                | ProviderCategory::DeepSeek
        )
    }

    /// Check if this is a major cloud platform (as opposed to direct model provider)
    pub fn is_cloud_platform(&self) -> bool {
        matches!(
            self,
            ProviderCategory::Amazon | ProviderCategory::Azure | ProviderCategory::VertexAI
        )
    }

    /// Get display name for UI
    pub fn display_name(&self) -> &str {
        match self {
            ProviderCategory::Anthropic => "Anthropic",
            ProviderCategory::OpenAI => "OpenAI",
            ProviderCategory::Google => "Google",
            ProviderCategory::XAI => "xAI",
            ProviderCategory::DeepSeek => "DeepSeek",
            ProviderCategory::Mistral => "Mistral AI",
            ProviderCategory::Cohere => "Cohere",
            ProviderCategory::Perplexity => "Perplexity",
            ProviderCategory::Alibaba => "Alibaba (Qwen)",
            ProviderCategory::Zhipu => "Zhipu AI (GLM)",
            ProviderCategory::Baidu => "Baidu (ERNIE)",
            ProviderCategory::Moonshot => "Moonshot (Kimi)",
            ProviderCategory::ByteDance => "ByteDance (Doubao)",
            ProviderCategory::Meta => "Meta",
            ProviderCategory::MetaLlama => "Meta (Llama)",
            ProviderCategory::Databricks => "Databricks",
            ProviderCategory::Stability => "Stability AI",
            ProviderCategory::Amazon => "Amazon Bedrock",
            ProviderCategory::Azure => "Azure OpenAI",
            ProviderCategory::VertexAI => "Google Vertex AI",
            ProviderCategory::Other => "Other",
        }
    }
}

/// ModelCategorySet represents multiple categories for a model.
#[derive(Debug, Clone)]
pub struct ModelCategorySet {
    pub capabilities: Vec<CapabilityCategory>,
    pub tier: TierCategory,
    pub cost: CostCategory,
    pub context: ContextWindowCategory,
    pub provider: ProviderCategory,
}

/// Extension trait to add categorization methods to ModelInfo
pub trait ModelCategorization {
    fn get_categories(&self) -> ModelCategorySet;
    fn get_capability_categories(&self) -> Vec<CapabilityCategory>;
    fn get_tier(&self) -> TierCategory;
    fn get_cost_category(&self) -> CostCategory;
    fn get_context_category(&self) -> ContextWindowCategory;
    fn get_provider_category(&self) -> ProviderCategory;
    fn has_all_capabilities(&self, caps: &[CapabilityCategory]) -> bool;
    fn has_any_capability(&self, caps: &[CapabilityCategory]) -> bool;
    fn is_in_tier(&self, tier: TierCategory) -> bool;
    fn is_in_cost_range(&self, cost: CostCategory) -> bool;
    fn is_in_context_range(&self, context: ContextWindowCategory) -> bool;
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
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        }
    }

    #[test]
    fn test_tier_categorization() {
        // Flagship model
        let flagship = create_test_model("claude-opus-4-20250514", "anthropic", 15.0, 200000);
        assert_eq!(flagship.get_tier(), TierCategory::Flagship);

        // Standard model
        let standard = create_test_model("claude-sonnet-4-20250514", "anthropic", 3.0, 200000);
        assert_eq!(standard.get_tier(), TierCategory::Standard);

        // Fast model
        let fast = create_test_model("gemini-2.5-flash", "google", 0.075, 1000000);
        assert_eq!(fast.get_tier(), TierCategory::Fast);
    }

    #[test]
    fn test_cost_categorization() {
        let ultra_premium = create_test_model("test", "test", 60.0, 100000);
        assert_eq!(
            ultra_premium.get_cost_category(),
            CostCategory::UltraPremium
        );

        let premium = create_test_model("test", "test", 15.0, 100000);
        assert_eq!(premium.get_cost_category(), CostCategory::Premium);

        let standard = create_test_model("test", "test", 3.0, 100000);
        assert_eq!(standard.get_cost_category(), CostCategory::Standard);

        let economy = create_test_model("test", "test", 0.5, 100000);
        assert_eq!(economy.get_cost_category(), CostCategory::Economy);
    }

    #[test]
    fn test_context_categorization() {
        let ultra = create_test_model("test", "test", 1.0, 1000000);
        assert_eq!(ultra.get_context_category(), ContextWindowCategory::Ultra);

        let large = create_test_model("test", "test", 1.0, 200000);
        assert_eq!(large.get_context_category(), ContextWindowCategory::Large);

        let medium = create_test_model("test", "test", 1.0, 100000);
        assert_eq!(medium.get_context_category(), ContextWindowCategory::Medium);

        let small = create_test_model("test", "test", 1.0, 16000);
        assert_eq!(small.get_context_category(), ContextWindowCategory::Small);
    }

    #[test]
    fn test_capability_checks() {
        let model = create_test_model("test", "test", 1.0, 100000);

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
