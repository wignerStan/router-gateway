use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// `ModelInfo` contains model metadata for routing decisions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelInfo {
    /// ID is the unique model identifier (e.g., "claude-sonnet-4-20250514")
    pub id: String,

    /// Name is the human-readable model name
    pub name: String,

    /// Provider is the model provider (e.g., "anthropic", "openai", "google")
    pub provider: String,

    /// `ContextWindow` is the maximum context window in tokens
    pub context_window: usize,

    /// `MaxOutputTokens` is the maximum output tokens
    pub max_output_tokens: usize,

    /// `InputPricePerMillion` is input price per 1M tokens in USD
    pub input_price_per_million: f64,

    /// `OutputPricePerMillion` is output price per 1M tokens in USD
    pub output_price_per_million: f64,

    /// Capabilities lists supported features
    pub capabilities: ModelCapabilities,

    /// `RateLimits` defines rate limiting constraints
    pub rate_limits: RateLimits,

    /// Source indicates where this data came from
    pub source: DataSource,
}

/// `ModelCapabilities` defines supported model features.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelCapabilities {
    /// Streaming indicates if streaming responses are supported
    pub streaming: bool,

    /// Tools indicates if function calling is supported
    pub tools: bool,

    /// Vision indicates if image/vision input is supported
    pub vision: bool,

    /// Thinking indicates if extended thinking is supported
    pub thinking: bool,
}

/// `RateLimits` defines rate limiting constraints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RateLimits {
    /// `RequestsPerMinute` is the rate limit for requests
    pub requests_per_minute: usize,

    /// `TokensPerMinute` is the rate limit for tokens
    pub tokens_per_minute: usize,
}

/// `DataSource` indicates where model data originated.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub enum DataSource {
    /// Static indicates hardcoded fallback data
    Static,
    /// `ModelsDev` indicates data from models.dev API
    ModelsDev,
    /// `LiteLLM` indicates data from `LiteLLM` proxy
    LiteLLM,
    /// Local indicates locally configured data
    Local,
}

impl fmt::Display for DataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static => write!(f, "static"),
            Self::ModelsDev => write!(f, "models.dev"),
            Self::LiteLLM => write!(f, "litellm"),
            Self::Local => write!(f, "local"),
        }
    }
}

/// Errors that can occur when working with `ModelInfo`
#[derive(Debug, Error)]
pub enum ModelInfoError {
    #[error("model ID cannot be empty")]
    EmptyId,
    #[error("invalid context window for model {model}: {window}", model = .0, window = .1)]
    InvalidContextWindow(String, usize),
    #[error("invalid input price for model {model}: {price}", model = .0, price = .1)]
    InvalidInputPrice(String, f64),
    #[error("invalid output price for model {model}: {price}", model = .0, price = .1)]
    InvalidOutputPrice(String, f64),
    #[error("invalid request rate limit for model {model}: {limit}", model = .0, limit = .1)]
    InvalidRequestRateLimit(String, usize),
    #[error("invalid token rate limit for model {model}: {limit}", model = .0, limit = .1)]
    InvalidTokenRateLimit(String, usize),
}

impl ModelInfo {
    /// `SupportsCapability` checks if the model supports a specific capability.
    pub fn supports_capability(&self, capability: &str) -> bool {
        match capability {
            "streaming" => self.capabilities.streaming,
            "tools" => self.capabilities.tools,
            "vision" => self.capabilities.vision,
            "thinking" => self.capabilities.thinking,
            _ => false,
        }
    }

    /// `EstimateCost` calculates the estimated cost for a request in USD.
    pub fn estimate_cost(&self, input_tokens: usize, output_tokens: usize) -> f64 {
        let input_cost = (input_tokens as f64) / 1_000_000.0 * self.input_price_per_million;
        let output_cost = (output_tokens as f64) / 1_000_000.0 * self.output_price_per_million;
        input_cost + output_cost
    }

    /// `CanFitContext` checks if the model can handle the given token count.
    pub const fn can_fit_context(&self, tokens: usize) -> bool {
        tokens > 0 && tokens <= self.context_window
    }

    /// `GetMaxTokens` returns the maximum output tokens allowed.
    pub fn get_max_tokens(&self) -> usize {
        if self.max_output_tokens > 0 && self.max_output_tokens < self.context_window {
            self.max_output_tokens
        } else {
            // Conservative estimate: 75% of context window for output
            (self.context_window as f64 * 0.75) as usize
        }
    }

    /// Validate checks if the model info is valid.
    pub fn validate(&self) -> Result<(), ModelInfoError> {
        if self.id.is_empty() {
            return Err(ModelInfoError::EmptyId);
        }
        if self.context_window == 0 {
            return Err(ModelInfoError::InvalidContextWindow(
                self.id.clone(),
                self.context_window,
            ));
        }
        if self.input_price_per_million < 0.0 {
            return Err(ModelInfoError::InvalidInputPrice(
                self.id.clone(),
                self.input_price_per_million,
            ));
        }
        if self.output_price_per_million < 0.0 {
            return Err(ModelInfoError::InvalidOutputPrice(
                self.id.clone(),
                self.output_price_per_million,
            ));
        }
        // Note: rate_limits of 0 is valid (means no limit), so we don't check for that

        Ok(())
    }
}

/// `EstimateRequestTokens` provides a rough token estimate for text input.
/// This is a simple heuristic: ~4 characters per token for English text.
pub fn estimate_request_tokens(text: &str) -> usize {
    // Rough estimate: 4 chars per token for English text
    // This is conservative for code and other content
    ((text.len() as f64) / 4.0).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_info_validation() {
        let info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 4096,
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: false,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        assert!(info.validate().is_ok());
    }

    #[test]
    fn test_model_info_empty_id() {
        let info = ModelInfo {
            id: "".to_string(),
            name: "Test".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 4096,
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: false,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        assert!(info.validate().is_err());
    }

    #[test]
    fn test_estimate_cost() {
        let info = ModelInfo {
            id: "test".to_string(),
            name: "Test".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 4096,
            input_price_per_million: 3.0,
            output_price_per_million: 15.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: false,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        let cost = info.estimate_cost(1_000_000, 500_000);
        assert!((cost - 10.5).abs() < 0.01); // 3.0 + 7.5 = 10.5
    }

    #[test]
    fn test_can_fit_context() {
        let info = ModelInfo {
            id: "test".to_string(),
            name: "Test".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 4096,
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: false,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        assert!(info.can_fit_context(100_000));
        assert!(info.can_fit_context(128_000));
        assert!(!info.can_fit_context(129_000));
        assert!(!info.can_fit_context(0));
    }

    #[test]
    fn test_estimate_request_tokens() {
        let text = "Hello, world!";
        let tokens = estimate_request_tokens(text);
        assert_eq!(tokens, 4); // 13 chars / 4 = 3.25 -> ceil = 4
    }

    // ========================================
    // ModelInfo Validation Error Paths
    // ========================================

    #[test]
    fn test_validate_invalid_context_window_zero() {
        let info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window: 0, // Invalid: zero context window
            max_output_tokens: 4096,
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: false,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        let result = info.validate();
        assert!(result.is_err());
        let err = result.expect_err("expected error should be present");
        assert!(matches!(err, ModelInfoError::InvalidContextWindow(_, 0)));
    }

    #[test]
    #[allow(clippy::panic)]
    fn test_validate_negative_input_price() {
        let info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 4096,
            input_price_per_million: -1.0, // Invalid: negative price
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: false,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        let result = info.validate();
        assert!(result.is_err());
        let err = result.expect_err("expected error should be present");
        // Check it's the right error type with the correct price
        if let ModelInfoError::InvalidInputPrice(model, price) = err {
            assert_eq!(model, "test-model");
            assert!((price - (-1.0)).abs() < 0.001);
        } else {
            panic!("Expected InvalidInputPrice error");
        }
    }

    #[test]
    #[allow(clippy::panic)]
    fn test_validate_negative_output_price() {
        let info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 4096,
            input_price_per_million: 1.0,
            output_price_per_million: -5.0, // Invalid: negative price
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: false,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        let result = info.validate();
        assert!(result.is_err());
        let err = result.expect_err("expected error should be present");
        // Check it's the right error type with the correct price
        if let ModelInfoError::InvalidOutputPrice(model, price) = err {
            assert_eq!(model, "test-model");
            assert!((price - (-5.0)).abs() < 0.001);
        } else {
            panic!("Expected InvalidOutputPrice error");
        }
    }

    #[test]
    fn test_validate_zero_price_is_valid() {
        // Zero prices are valid (free models)
        let info = ModelInfo {
            id: "free-model".to_string(),
            name: "Free Model".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 4096,
            input_price_per_million: 0.0,  // Valid: free model
            output_price_per_million: 0.0, // Valid: free model
            capabilities: ModelCapabilities {
                streaming: true,
                tools: false,
                vision: false,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        assert!(info.validate().is_ok());
    }

    // ========================================
    // supports_capability tests
    // ========================================

    #[test]
    fn test_supports_capability_unknown() {
        let info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 4096,
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: true,
                thinking: true,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        // Unknown capability should return false
        assert!(!info.supports_capability("unknown_capability"));
        assert!(!info.supports_capability("audio"));
        assert!(!info.supports_capability("embedding"));
        assert!(!info.supports_capability("function_calling")); // Wrong name
    }

    #[test]
    fn test_supports_capability_case_sensitivity() {
        let info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 4096,
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: true,
                thinking: true,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        // Capability matching is case-sensitive
        assert!(!info.supports_capability("STREAMING"));
        assert!(!info.supports_capability("Streaming"));
        assert!(!info.supports_capability("VISION"));
        assert!(!info.supports_capability("Tools"));
        assert!(info.supports_capability("streaming")); // Correct lowercase
        assert!(info.supports_capability("tools"));
        assert!(info.supports_capability("vision"));
        assert!(info.supports_capability("thinking"));
    }

    #[test]
    fn test_supports_capability_all_known_capabilities() {
        let info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 4096,
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: true,
                thinking: true,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        assert!(info.supports_capability("streaming"));
        assert!(info.supports_capability("tools"));
        assert!(info.supports_capability("vision"));
        assert!(info.supports_capability("thinking"));
    }

    // ========================================
    // can_fit_context edge cases
    // ========================================

    #[test]
    fn test_can_fit_context_zero_tokens() {
        let info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 4096,
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: false,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        // Zero tokens should return false (not a valid request)
        assert!(!info.can_fit_context(0));
    }

    #[test]
    fn test_can_fit_context_exact_boundary() {
        let info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 4096,
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: false,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        // Exact boundary cases
        assert!(info.can_fit_context(128_000)); // Exactly at limit - should fit
        assert!(info.can_fit_context(127_999)); // One less than limit
        assert!(!info.can_fit_context(128_001)); // One more than limit
    }

    #[test]
    fn test_can_fit_context_one_token() {
        let info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 4096,
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: false,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        // Single token should fit
        assert!(info.can_fit_context(1));
    }

    // ========================================
    // get_max_tokens edge cases
    // ========================================

    #[test]
    fn test_get_max_tokens_when_set_smaller_than_context() {
        let info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 4096, // Smaller than context window
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: false,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        // Should return max_output_tokens when it's smaller than context window
        assert_eq!(info.get_max_tokens(), 4096);
    }

    #[test]
    fn test_get_max_tokens_when_zero() {
        let info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 0, // Zero means use fallback
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: false,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        // Should return 75% of context window as fallback
        let expected = (128_000_f64 * 0.75) as usize;
        assert_eq!(info.get_max_tokens(), expected);
    }

    #[test]
    fn test_get_max_tokens_when_larger_than_context() {
        let info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 200_000, // Larger than context window
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: false,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        // Should return 75% of context window when max_output_tokens >= context_window
        let expected = (128_000_f64 * 0.75) as usize;
        assert_eq!(info.get_max_tokens(), expected);
    }

    #[test]
    fn test_get_max_tokens_equal_to_context() {
        let info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 128_000, // Equal to context window
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: false,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        // Should return 75% fallback when max_output_tokens >= context_window
        let expected = (128_000_f64 * 0.75) as usize;
        assert_eq!(info.get_max_tokens(), expected);
    }

    #[test]
    fn test_get_max_tokens_just_under_context() {
        let info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window: 128_000,
            max_output_tokens: 127_999, // Just under context window
            input_price_per_million: 1.0,
            output_price_per_million: 2.0,
            capabilities: ModelCapabilities {
                streaming: true,
                tools: true,
                vision: false,
                thinking: false,
            },
            rate_limits: RateLimits {
                requests_per_minute: 60,
                tokens_per_minute: 90000,
            },
            source: DataSource::Static,
        };

        // Should return actual max_output_tokens
        assert_eq!(info.get_max_tokens(), 127_999);
    }

    // ========================================
    // Error message formatting tests
    // ========================================

    #[test]
    fn test_model_info_error_display() {
        let err = ModelInfoError::EmptyId;
        assert_eq!(err.to_string(), "model ID cannot be empty");

        let err = ModelInfoError::InvalidContextWindow("test-model".to_string(), 0);
        assert!(err.to_string().contains("test-model"));
        assert!(err.to_string().contains("0"));

        let err = ModelInfoError::InvalidInputPrice("test-model".to_string(), -1.5);
        assert!(err.to_string().contains("test-model"));
        assert!(err.to_string().contains("-1.5"));
    }
}
