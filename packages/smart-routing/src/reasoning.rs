use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Reasoning capability inference for models
///
/// Infers thinking/reasoning capability from model hints,
/// explicit flags, and model family detection.
/// Reasoning capability level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasoningCapability {
    /// No reasoning capability
    None,
    /// Basic reasoning capability
    Basic,
    /// Extended reasoning capability (o1, o1-mini, etc.)
    Extended,
    /// High-reasoning capability (o1-pro, claude-opus, etc.)
    High,
}

/// Reasoning request context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningRequest {
    /// Model identifier (e.g., "claude-3-opus", "o1-preview")
    pub model: String,
    /// Explicit reasoning flag in request
    pub reasoning_flag: Option<bool>,
    /// Max tokens hint (can indicate reasoning requests)
    pub max_tokens: Option<u32>,
    /// Additional model hints or metadata
    pub hints: HashMap<String, String>,
}

/// Reasoning capability inference engine
pub struct ReasoningInference {
    /// Cache of model capabilities
    model_cache: tokio::sync::RwLock<HashMap<String, ReasoningCapability>>,
    /// Known reasoning-optimized model families
    reasoning_families: Vec<String>,
}

impl Default for ReasoningInference {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ReasoningInference {
    fn clone(&self) -> Self {
        Self {
            model_cache: tokio::sync::RwLock::new(HashMap::new()),
            reasoning_families: self.reasoning_families.clone(),
        }
    }
}

impl ReasoningInference {
    /// Create a new reasoning inference engine
    pub fn new() -> Self {
        Self {
            model_cache: tokio::sync::RwLock::new(HashMap::new()),
            reasoning_families: vec![
                // OpenAI o1 series
                "o1".to_string(),
                "o1-".to_string(),
                // Anthropic extended thinking
                "claude-thinking".to_string(),
                // Future reasoning families
                "reasoning".to_string(),
            ],
        }
    }

    /// Infer reasoning capability from request context
    pub async fn infer_capability(&self, request: &ReasoningRequest) -> ReasoningCapability {
        // 1. Check explicit reasoning flag
        if let Some(flag) = request.reasoning_flag {
            return if flag {
                ReasoningCapability::Extended
            } else {
                ReasoningCapability::None
            };
        }

        // 2. Check cache for known model capability
        {
            let cache = self.model_cache.read().await;
            if let Some(&capability) = cache.get(&request.model) {
                return capability;
            }
        }

        // 3. Infer from model family
        let capability = self.infer_from_model_family(&request.model);

        // 4. Enhance based on max_tokens hint
        let capability = self.enhance_from_tokens(capability, request.max_tokens);

        // 5. Cache the inferred capability
        {
            let mut cache = self.model_cache.write().await;
            cache.insert(request.model.clone(), capability);
        }

        capability
    }

    /// Infer capability from model family identifier
    fn infer_from_model_family(&self, model: &str) -> ReasoningCapability {
        let model_lower = model.to_lowercase();

        // OpenAI o1 series - high reasoning
        if model_lower.starts_with("o1") {
            if model_lower.contains("pro") || model_lower.ends_with("preview") {
                return ReasoningCapability::High;
            }
            return ReasoningCapability::Extended;
        }

        // Check for reasoning family hints
        for family in &self.reasoning_families {
            if model_lower.contains(family) {
                return ReasoningCapability::Extended;
            }
        }

        // Anthropic claude models - check for thinking designation
        if model_lower.contains("claude") && model_lower.contains("thinking") {
            return ReasoningCapability::Extended;
        }

        // Default: no reasoning capability
        ReasoningCapability::None
    }

    /// Enhance capability inference from `max_tokens` hint
    fn enhance_from_tokens(
        &self,
        base_capability: ReasoningCapability,
        max_tokens: Option<u32>,
    ) -> ReasoningCapability {
        // Very high max_tokens can indicate reasoning workloads
        if let Some(tokens) = max_tokens {
            if tokens >= 100_000 && base_capability == ReasoningCapability::None {
                // Large output context suggests potential reasoning
                return ReasoningCapability::Basic;
            }
        }

        base_capability
    }

    /// Check if a request requires reasoning capability
    pub async fn requires_reasoning(&self, request: &ReasoningRequest) -> bool {
        let capability = self.infer_capability(request).await;
        matches!(
            capability,
            ReasoningCapability::Basic | ReasoningCapability::Extended | ReasoningCapability::High
        )
    }

    /// Get the capability level (0-3, higher is better)
    pub const fn capability_level(capability: ReasoningCapability) -> u8 {
        match capability {
            ReasoningCapability::None => 0,
            ReasoningCapability::Basic => 1,
            ReasoningCapability::Extended => 2,
            ReasoningCapability::High => 3,
        }
    }

    /// Clear the model capability cache
    pub async fn clear_cache(&self) {
        let mut cache = self.model_cache.write().await;
        cache.clear();
    }

    /// Pre-seed cache with known model capabilities
    pub async fn seed_capability(&self, model: String, capability: ReasoningCapability) {
        let mut cache = self.model_cache.write().await;
        cache.insert(model, capability);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reasoning_capability_levels() {
        assert_eq!(
            ReasoningInference::capability_level(ReasoningCapability::None),
            0
        );
        assert_eq!(
            ReasoningInference::capability_level(ReasoningCapability::Basic),
            1
        );
        assert_eq!(
            ReasoningInference::capability_level(ReasoningCapability::Extended),
            2
        );
        assert_eq!(
            ReasoningInference::capability_level(ReasoningCapability::High),
            3
        );
    }

    #[tokio::test]
    async fn test_infer_o1_preview() {
        let inference = ReasoningInference::new();
        let request = ReasoningRequest {
            model: "o1-preview".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };

        let capability = inference.infer_capability(&request).await;
        assert_eq!(capability, ReasoningCapability::High);
    }

    #[tokio::test]
    async fn test_infer_o1_mini() {
        let inference = ReasoningInference::new();
        let request = ReasoningRequest {
            model: "o1-mini".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };

        let capability = inference.infer_capability(&request).await;
        assert_eq!(capability, ReasoningCapability::Extended);
    }

    #[tokio::test]
    async fn test_explicit_reasoning_flag() {
        let inference = ReasoningInference::new();

        // Explicit reasoning flag enabled
        let request_enabled = ReasoningRequest {
            model: "gpt-4".to_string(),
            reasoning_flag: Some(true),
            max_tokens: None,
            hints: HashMap::new(),
        };

        let capability = inference.infer_capability(&request_enabled).await;
        assert_eq!(capability, ReasoningCapability::Extended);

        // Explicit reasoning flag disabled
        let request_disabled = ReasoningRequest {
            model: "o1-preview".to_string(),
            reasoning_flag: Some(false),
            max_tokens: None,
            hints: HashMap::new(),
        };

        let capability = inference.infer_capability(&request_disabled).await;
        assert_eq!(capability, ReasoningCapability::None);
    }

    #[tokio::test]
    async fn test_claude_thinking_model() {
        let inference = ReasoningInference::new();
        let request = ReasoningRequest {
            model: "claude-3-5-thinking".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };

        let capability = inference.infer_capability(&request).await;
        assert_eq!(capability, ReasoningCapability::Extended);
    }

    #[tokio::test]
    async fn test_standard_model_no_reasoning() {
        let inference = ReasoningInference::new();
        let request = ReasoningRequest {
            model: "gpt-4".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };

        let capability = inference.infer_capability(&request).await;
        assert_eq!(capability, ReasoningCapability::None);
    }

    #[tokio::test]
    async fn test_max_tokens_hint() {
        let inference = ReasoningInference::new();
        let request = ReasoningRequest {
            model: "gpt-4".to_string(),
            reasoning_flag: None,
            max_tokens: Some(150_000),
            hints: HashMap::new(),
        };

        let capability = inference.infer_capability(&request).await;
        assert_eq!(capability, ReasoningCapability::Basic);
    }

    #[tokio::test]
    async fn test_requires_reasoning() {
        let inference = ReasoningInference::new();

        let reasoning_request = ReasoningRequest {
            model: "o1-preview".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };

        assert!(inference.requires_reasoning(&reasoning_request).await);

        let standard_request = ReasoningRequest {
            model: "gpt-4".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };

        assert!(!inference.requires_reasoning(&standard_request).await);
    }

    #[tokio::test]
    async fn test_cache_functionality() {
        let inference = ReasoningInference::new();

        // First call - populates cache
        let request = ReasoningRequest {
            model: "o1-preview".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };

        let capability1 = inference.infer_capability(&request).await;
        assert_eq!(capability1, ReasoningCapability::High);

        // Second call - uses cache
        let capability2 = inference.infer_capability(&request).await;
        assert_eq!(capability2, ReasoningCapability::High);
    }

    #[tokio::test]
    async fn test_seed_capability() {
        let inference = ReasoningInference::new();

        // Seed a custom capability
        inference
            .seed_capability("custom-model".to_string(), ReasoningCapability::Basic)
            .await;

        let request = ReasoningRequest {
            model: "custom-model".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };

        let capability = inference.infer_capability(&request).await;
        assert_eq!(capability, ReasoningCapability::Basic);
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let inference = ReasoningInference::new();

        // Populate cache
        inference
            .seed_capability("test-model".to_string(), ReasoningCapability::Extended)
            .await;

        inference.clear_cache().await;

        // After clear, inference should re-evaluate
        let request = ReasoningRequest {
            model: "test-model".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };

        let capability = inference.infer_capability(&request).await;
        // Should re-infer as None since test-model doesn't match known patterns
        assert_eq!(capability, ReasoningCapability::None);
    }

    #[tokio::test]
    async fn test_clone_independence() {
        let inference1 = ReasoningInference::new();

        // Seed capability in original
        inference1
            .seed_capability("test-model".to_string(), ReasoningCapability::Extended)
            .await;

        let inference2 = inference1.clone();

        // Clone should have independent storage
        let request = ReasoningRequest {
            model: "test-model".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };

        // inference2 should not have the seeded entry
        let capability = inference2.infer_capability(&request).await;
        assert_eq!(capability, ReasoningCapability::None);
    }

    #[tokio::test]
    async fn test_o1_pro_high_reasoning() {
        let inference = ReasoningInference::new();
        let request = ReasoningRequest {
            model: "o1-pro".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };

        let capability = inference.infer_capability(&request).await;
        assert_eq!(capability, ReasoningCapability::High);
    }

    #[tokio::test]
    async fn test_empty_model() {
        let inference = ReasoningInference::new();
        let request = ReasoningRequest {
            model: "".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };

        let capability = inference.infer_capability(&request).await;
        assert_eq!(capability, ReasoningCapability::None);
    }
}
