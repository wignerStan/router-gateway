use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Content type detection for LLM requests.
pub mod detection;
/// Request format and protocol detection.
pub mod format;
/// Token estimation for LLM requests.
pub mod token;

pub use detection::{ContentTypeDetector, StreamingExtractor, ToolDetector};
pub use format::FormatDetector;
pub use token::TokenEstimator;

/// Trait for classifying LLM requests to extract routing-relevant information
pub trait RequestClassifier: Send + Sync {
    /// Classify a request and extract routing-relevant information
    fn classify(&self, request: &Value) -> ClassifiedRequest;
}

/// Result of classifying a request, containing all routing-relevant information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClassifiedRequest {
    /// Required capabilities for this request
    pub required_capabilities: RequiredCapabilities,
    /// Estimated number of tokens (0 = unknown/unspecified)
    pub estimated_tokens: u32,
    /// Request format/protocol
    pub format: RequestFormat,
    /// Quality/speed preference
    pub quality_preference: QualityPreference,
}

/// Required capabilities for a request
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RequiredCapabilities {
    /// Vision/image processing required
    pub vision: bool,
    /// Tool/function calling required
    pub tools: bool,
    /// Streaming response required
    pub streaming: bool,
    /// Thinking/reasoning capability required
    pub thinking: bool,
}

/// Request format/protocol
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RequestFormat {
    /// OpenAI-compatible format
    OpenAI,
    /// Anthropic format
    Anthropic,
    /// Gemini format
    Gemini,
    /// Generic/unknown format
    Generic,
}

/// Quality/speed preference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum QualityPreference {
    /// Prioritize speed over quality
    Speed,
    /// Balance between speed and quality
    Balanced,
    /// Prioritize quality over speed
    Quality,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_required_capabilities_default() {
        let caps = RequiredCapabilities::default();
        assert!(!caps.vision);
        assert!(!caps.tools);
        assert!(!caps.streaming);
        assert!(!caps.thinking);
    }

    #[test]
    fn test_classified_request_creation() {
        let request = ClassifiedRequest {
            required_capabilities: RequiredCapabilities {
                vision: true,
                tools: false,
                streaming: true,
                thinking: false,
            },
            estimated_tokens: 1000,
            format: RequestFormat::OpenAI,
            quality_preference: QualityPreference::Balanced,
        };

        assert!(request.required_capabilities.vision);
        assert!(request.required_capabilities.streaming);
        assert_eq!(request.estimated_tokens, 1000);
        assert_eq!(request.format, RequestFormat::OpenAI);
    }

    #[test]
    fn test_request_format_equality() {
        assert_eq!(RequestFormat::OpenAI, RequestFormat::OpenAI);
        assert_ne!(RequestFormat::OpenAI, RequestFormat::Anthropic);
    }

    #[test]
    fn test_quality_preference_equality() {
        assert_eq!(QualityPreference::Speed, QualityPreference::Speed);
        assert_ne!(QualityPreference::Speed, QualityPreference::Quality);
    }
}
