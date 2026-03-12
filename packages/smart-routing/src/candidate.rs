//! Route candidate construction for intelligent routing
//!
//! This module builds route candidates from available credentials and models,
//! providing the foundation for intelligent credential selection.

use crate::classification::{ClassifiedRequest, RequiredCapabilities};
use model_registry::ModelInfo;
use std::collections::HashMap;

/// Route candidate representing a potential routing option
///
/// Contains all information needed to evaluate and select a credential
/// for handling an LLM request.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteCandidate {
    /// Unique credential identifier
    pub credential_id: String,
    /// Model identifier
    pub model_id: String,
    /// Provider name
    pub provider: String,
    /// Model information
    pub model_info: ModelInfo,
    /// Estimated cost in USD (0.0 = unknown)
    pub estimated_cost: f64,
    /// Token fit status
    pub token_fit: TokenFitStatus,
}

/// Token fit status for context window checking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenFitStatus {
    /// Request fits within context window
    Fits,
    /// Request exceeds context window
    Exceeds,
    /// Unknown (token count not estimated)
    Unknown,
}

/// Builder for route candidates
#[derive(Debug, Clone)]
pub struct CandidateBuilder {
    /// Credential ID to model ID mappings
    credential_models: HashMap<String, Vec<String>>,
    /// Model registry for looking up model info
    models: HashMap<String, ModelInfo>,
}

impl CandidateBuilder {
    /// Create a new candidate builder
    pub fn new() -> Self {
        Self {
            credential_models: HashMap::new(),
            models: HashMap::new(),
        }
    }

    /// Add a credential with its associated models
    pub fn add_credential(&mut self, credential_id: String, model_ids: Vec<String>) -> &mut Self {
        self.credential_models.insert(credential_id, model_ids);
        self
    }

    /// Set model information
    pub fn set_model(&mut self, model_id: String, info: ModelInfo) -> &mut Self {
        self.models.insert(model_id, info);
        self
    }

    /// Build route candidates for a request
    ///
    /// # Scenarios
    /// - Valid model creates candidates
    /// - No credentials returns empty list
    /// - Multiple credentials creates multiple candidates
    pub fn build_candidates(&self, request: &ClassifiedRequest) -> Vec<RouteCandidate> {
        let mut candidates = Vec::new();

        // No credentials → empty list
        if self.credential_models.is_empty() {
            return candidates;
        }

        // Build candidates from each credential-model pair
        for (credential_id, model_ids) in &self.credential_models {
            for model_id in model_ids {
                if let Some(model_info) = self.models.get(model_id) {
                    // Check token fit
                    let token_fit = if request.estimated_tokens > 0 {
                        if model_info.can_fit_context(request.estimated_tokens as usize) {
                            TokenFitStatus::Fits
                        } else {
                            TokenFitStatus::Exceeds
                        }
                    } else {
                        TokenFitStatus::Unknown
                    };

                    let candidate = RouteCandidate {
                        credential_id: credential_id.clone(),
                        model_id: model_id.clone(),
                        provider: model_info.provider.clone(),
                        model_info: model_info.clone(),
                        estimated_cost: 0.0, // Calculated during filtering
                        token_fit,
                    };

                    candidates.push(candidate);
                }
            }
        }

        candidates
    }
}

impl Default for CandidateBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if required capabilities are supported by a model
pub fn check_capability_support(
    required: &RequiredCapabilities,
    model_info: &ModelInfo,
) -> CapabilitySupport {
    let capabilities = &model_info.capabilities;

    let vision_match = !required.vision || capabilities.vision;
    let tools_match = !required.tools || capabilities.tools;
    let streaming_match = !required.streaming || capabilities.streaming;
    let thinking_match = !required.thinking || capabilities.thinking;

    if vision_match && tools_match && streaming_match && thinking_match {
        CapabilitySupport::Supported
    } else {
        CapabilitySupport::Unsupported {
            missing_vision: required.vision && !capabilities.vision,
            missing_tools: required.tools && !capabilities.tools,
            missing_streaming: required.streaming && !capabilities.streaming,
            missing_thinking: required.thinking && !capabilities.thinking,
        }
    }
}

/// Capability support status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilitySupport {
    /// All required capabilities are supported
    Supported,
    /// Some required capabilities are not supported
    Unsupported {
        missing_vision: bool,
        missing_tools: bool,
        missing_streaming: bool,
        missing_thinking: bool,
    },
}

impl CapabilitySupport {
    /// Check if capabilities are fully supported
    pub fn is_supported(&self) -> bool {
        matches!(self, CapabilitySupport::Supported)
    }

    /// Get description of missing capabilities
    pub fn missing_description(&self) -> Option<String> {
        match self {
            CapabilitySupport::Supported => None,
            CapabilitySupport::Unsupported {
                missing_vision,
                missing_tools,
                missing_streaming,
                missing_thinking,
            } => {
                let mut missing = Vec::new();
                if *missing_vision {
                    missing.push("vision");
                }
                if *missing_tools {
                    missing.push("tools");
                }
                if *missing_streaming {
                    missing.push("streaming");
                }
                if *missing_thinking {
                    missing.push("thinking");
                }
                if missing.is_empty() {
                    None
                } else {
                    Some(missing.join(", "))
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model_registry::{DataSource, ModelCapabilities, RateLimits};

    fn create_test_model(id: &str, provider: &str, context_window: usize) -> ModelInfo {
        ModelInfo {
            id: id.to_string(),
            name: format!("Test Model {}", id),
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

    #[test]
    fn test_candidate_builder_empty() {
        let builder = CandidateBuilder::new();
        let request = ClassifiedRequest {
            required_capabilities: RequiredCapabilities::default(),
            estimated_tokens: 1000,
            format: crate::classification::RequestFormat::OpenAI,
            quality_preference: crate::classification::QualityPreference::Balanced,
        };

        let candidates = builder.build_candidates(&request);
        assert!(
            candidates.is_empty(),
            "No credentials should produce empty list"
        );
    }

    #[test]
    fn test_candidate_builder_valid_model() {
        let mut builder = CandidateBuilder::new();
        builder.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        builder.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );

        let request = ClassifiedRequest {
            required_capabilities: RequiredCapabilities::default(),
            estimated_tokens: 1000,
            format: crate::classification::RequestFormat::OpenAI,
            quality_preference: crate::classification::QualityPreference::Balanced,
        };

        let candidates = builder.build_candidates(&request);
        assert_eq!(candidates.len(), 1, "Should create one candidate");
        assert_eq!(candidates[0].credential_id, "cred-1");
        assert_eq!(candidates[0].model_id, "claude-3-opus");
        assert_eq!(candidates[0].provider, "anthropic");
    }

    #[test]
    fn test_candidate_builder_multiple_credentials() {
        let mut builder = CandidateBuilder::new();
        builder.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        builder.add_credential("cred-2".to_string(), vec!["gpt-4".to_string()]);
        builder.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );
        builder.set_model(
            "gpt-4".to_string(),
            create_test_model("gpt-4", "openai", 128000),
        );

        let request = ClassifiedRequest {
            required_capabilities: RequiredCapabilities::default(),
            estimated_tokens: 1000,
            format: crate::classification::RequestFormat::OpenAI,
            quality_preference: crate::classification::QualityPreference::Balanced,
        };

        let candidates = builder.build_candidates(&request);
        assert_eq!(candidates.len(), 2, "Should create two candidates");
    }

    #[test]
    fn test_candidate_builder_credential_with_multiple_models() {
        let mut builder = CandidateBuilder::new();
        builder.add_credential(
            "cred-1".to_string(),
            vec!["claude-3-opus".to_string(), "claude-3-sonnet".to_string()],
        );
        builder.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );
        builder.set_model(
            "claude-3-sonnet".to_string(),
            create_test_model("claude-3-sonnet", "anthropic", 200000),
        );

        let request = ClassifiedRequest {
            required_capabilities: RequiredCapabilities::default(),
            estimated_tokens: 1000,
            format: crate::classification::RequestFormat::OpenAI,
            quality_preference: crate::classification::QualityPreference::Balanced,
        };

        let candidates = builder.build_candidates(&request);
        assert_eq!(
            candidates.len(),
            2,
            "Should create two candidates for one credential"
        );
    }

    #[test]
    fn test_candidate_builder_unknown_model() {
        let mut builder = CandidateBuilder::new();
        builder.add_credential("cred-1".to_string(), vec!["unknown-model".to_string()]);

        let request = ClassifiedRequest {
            required_capabilities: RequiredCapabilities::default(),
            estimated_tokens: 1000,
            format: crate::classification::RequestFormat::OpenAI,
            quality_preference: crate::classification::QualityPreference::Balanced,
        };

        let candidates = builder.build_candidates(&request);
        assert!(
            candidates.is_empty(),
            "Unknown model should not create candidates"
        );
    }

    #[test]
    fn test_token_fit_status_fits() {
        let mut builder = CandidateBuilder::new();
        builder.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        builder.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );

        let request = ClassifiedRequest {
            required_capabilities: RequiredCapabilities::default(),
            estimated_tokens: 100000, // Fits within 200k
            format: crate::classification::RequestFormat::OpenAI,
            quality_preference: crate::classification::QualityPreference::Balanced,
        };

        let candidates = builder.build_candidates(&request);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].token_fit, TokenFitStatus::Fits);
    }

    #[test]
    fn test_token_fit_status_exceeds() {
        let mut builder = CandidateBuilder::new();
        builder.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        builder.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );

        let request = ClassifiedRequest {
            required_capabilities: RequiredCapabilities::default(),
            estimated_tokens: 300000, // Exceeds 200k
            format: crate::classification::RequestFormat::OpenAI,
            quality_preference: crate::classification::QualityPreference::Balanced,
        };

        let candidates = builder.build_candidates(&request);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].token_fit, TokenFitStatus::Exceeds);
    }

    #[test]
    fn test_token_fit_status_unknown() {
        let mut builder = CandidateBuilder::new();
        builder.add_credential("cred-1".to_string(), vec!["claude-3-opus".to_string()]);
        builder.set_model(
            "claude-3-opus".to_string(),
            create_test_model("claude-3-opus", "anthropic", 200000),
        );

        let request = ClassifiedRequest {
            required_capabilities: RequiredCapabilities::default(),
            estimated_tokens: 0, // Unknown
            format: crate::classification::RequestFormat::OpenAI,
            quality_preference: crate::classification::QualityPreference::Balanced,
        };

        let candidates = builder.build_candidates(&request);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].token_fit, TokenFitStatus::Unknown);
    }

    #[test]
    fn test_check_capability_support_all_supported() {
        let required = RequiredCapabilities {
            vision: true,
            tools: true,
            streaming: true,
            thinking: false,
        };

        let model = create_test_model("test", "test", 128000);

        let support = check_capability_support(&required, &model);
        assert!(support.is_supported());
        assert!(support.missing_description().is_none());
    }

    #[test]
    fn test_check_capability_support_missing_vision() {
        let required = RequiredCapabilities {
            vision: true,
            tools: false,
            streaming: false,
            thinking: false,
        };

        let mut model = create_test_model("test", "test", 128000);
        model.capabilities.vision = false;

        let support = check_capability_support(&required, &model);
        assert!(!support.is_supported());
        assert_eq!(support.missing_description(), Some("vision".to_string()));
    }

    #[test]
    fn test_check_capability_support_multiple_missing() {
        let required = RequiredCapabilities {
            vision: true,
            tools: true,
            streaming: true,
            thinking: true,
        };

        let mut model = create_test_model("test", "test", 128000);
        model.capabilities.vision = false;
        model.capabilities.thinking = false;

        let support = check_capability_support(&required, &model);
        assert!(!support.is_supported());
        let missing = support.missing_description();
        assert!(missing.is_some());
        let desc = missing.unwrap();
        assert!(desc.contains("vision"));
        assert!(desc.contains("thinking"));
    }

    #[test]
    fn test_check_capability_support_none_required() {
        let required = RequiredCapabilities {
            vision: false,
            tools: false,
            streaming: false,
            thinking: false,
        };

        let model = create_test_model("test", "test", 128000);

        let support = check_capability_support(&required, &model);
        assert!(support.is_supported());
    }

    #[test]
    fn test_route_candidate_clone() {
        let candidate = RouteCandidate {
            credential_id: "cred-1".to_string(),
            model_id: "claude-3-opus".to_string(),
            provider: "anthropic".to_string(),
            model_info: create_test_model("claude-3-opus", "anthropic", 200000),
            estimated_cost: 0.01,
            token_fit: TokenFitStatus::Fits,
        };

        let cloned = candidate.clone();
        assert_eq!(candidate, cloned);
    }

    #[test]
    fn test_token_fit_status_equality() {
        assert_eq!(TokenFitStatus::Fits, TokenFitStatus::Fits);
        assert_eq!(TokenFitStatus::Exceeds, TokenFitStatus::Exceeds);
        assert_eq!(TokenFitStatus::Unknown, TokenFitStatus::Unknown);
        assert_ne!(TokenFitStatus::Fits, TokenFitStatus::Exceeds);
        assert_ne!(TokenFitStatus::Fits, TokenFitStatus::Unknown);
    }

    #[test]
    fn test_capability_support_equality() {
        let supported = CapabilitySupport::Supported;
        let unsupported = CapabilitySupport::Unsupported {
            missing_vision: true,
            missing_tools: false,
            missing_streaming: false,
            missing_thinking: false,
        };

        assert_eq!(supported, CapabilitySupport::Supported);
        assert_eq!(
            unsupported,
            CapabilitySupport::Unsupported {
                missing_vision: true,
                missing_tools: false,
                missing_streaming: false,
                missing_thinking: false,
            }
        );
        assert_ne!(supported, unsupported);
    }
}
