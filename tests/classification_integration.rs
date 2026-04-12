#![allow(clippy::unreadable_literal, missing_docs)]
// Integration tests for request classification
//
// Unit-level integration tests covering vision detection, tool detection,
// streaming extraction, format detection, token estimation, and reasoning.
// Behavioral BDD coverage lives in tests/bdd/classification.rs (cucumber).

#[cfg(test)]
mod classification {

    #[tokio::test]
    async fn test_vision_detection_image_url() {
        use gateway::routing::classification::ContentTypeDetector;

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
    }

    #[tokio::test]
    async fn test_vision_detection_text_only() {
        use gateway::routing::classification::ContentTypeDetector;

        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": "Hello, world!"
            }]
        });
        assert!(!ContentTypeDetector::detect_vision_required(&request));
    }

    #[tokio::test]
    async fn test_vision_detection_mixed_content() {
        use gateway::routing::classification::ContentTypeDetector;

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
    async fn test_tool_detection_with_tools() {
        use gateway::routing::classification::ToolDetector;

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
    }

    #[tokio::test]
    async fn test_tool_detection_without_tools() {
        use gateway::routing::classification::ToolDetector;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });
        assert!(!ToolDetector::detect_tools_required(&request));
    }

    #[tokio::test]
    async fn test_tool_detection_empty_array() {
        use gateway::routing::classification::ToolDetector;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "tools": []
        });
        assert!(!ToolDetector::detect_tools_required(&request));
    }

    #[tokio::test]
    async fn test_streaming_detection_enabled() {
        use gateway::routing::classification::StreamingExtractor;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": true
        });
        assert!(StreamingExtractor::extract_streaming_preference(&request));
    }

    #[tokio::test]
    async fn test_streaming_detection_disabled() {
        use gateway::routing::classification::StreamingExtractor;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": false
        });
        assert!(!StreamingExtractor::extract_streaming_preference(&request));
    }

    #[tokio::test]
    async fn test_streaming_detection_absent() {
        use gateway::routing::classification::StreamingExtractor;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });
        assert!(!StreamingExtractor::extract_streaming_preference(&request));
    }

    #[tokio::test]
    async fn test_format_detection_openai() {
        use gateway::routing::classification::FormatDetector;
        use gateway::routing::classification::RequestFormat;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "model": "gpt-4"
        });
        assert_eq!(FormatDetector::detect(&request), RequestFormat::OpenAI);
    }

    #[tokio::test]
    async fn test_format_detection_anthropic() {
        use gateway::routing::classification::FormatDetector;
        use gateway::routing::classification::RequestFormat;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "system": "You are a helpful assistant",
            "model": "claude-3-opus"
        });
        assert_eq!(FormatDetector::detect(&request), RequestFormat::Anthropic);
    }

    #[tokio::test]
    async fn test_format_detection_gemini() {
        use gateway::routing::classification::FormatDetector;
        use gateway::routing::classification::RequestFormat;

        let request = serde_json::json!({
            "contents": [{"parts": [{"text": "Hello"}]}],
            "model": "gemini-pro"
        });
        assert_eq!(FormatDetector::detect(&request), RequestFormat::Gemini);
    }

    #[tokio::test]
    async fn test_format_detection_unknown() {
        use gateway::routing::classification::FormatDetector;
        use gateway::routing::classification::RequestFormat;

        let request = serde_json::json!({
            "prompt": "Hello",
            "model": "unknown-model"
        });
        assert_eq!(FormatDetector::detect(&request), RequestFormat::Generic);
    }

    #[tokio::test]
    async fn test_token_estimation_small_prompt() {
        use gateway::routing::classification::TokenEstimator;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });
        let tokens = TokenEstimator::estimate(&request);
        assert!(tokens < 1000, "Small prompt should fit standard context");
    }

    #[tokio::test]
    async fn test_token_estimation_large_prompt() {
        use gateway::routing::classification::TokenEstimator;

        let large_text = "x".repeat(200000); // ~50000 tokens
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": large_text}]
        });
        let tokens = TokenEstimator::estimate(&request);
        assert!(tokens > 40000, "Large prompt should require high context");
    }

    #[tokio::test]
    async fn test_token_estimation_combined_input_output() {
        use gateway::routing::classification::TokenEstimator;

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
    async fn test_reasoning_detection_explicit_flag() {
        use gateway::routing::reasoning::{ReasoningInference, ReasoningRequest};
        use std::collections::HashMap;

        let inference = ReasoningInference::new();
        let request = ReasoningRequest {
            model: "gpt-4".to_string(),
            reasoning_flag: Some(true),
            max_tokens: None,
            hints: HashMap::new(),
        };
        assert!(inference.requires_reasoning(&request).await);
    }

    #[tokio::test]
    async fn test_reasoning_detection_model_family() {
        use gateway::routing::reasoning::{ReasoningInference, ReasoningRequest};
        use std::collections::HashMap;

        let inference = ReasoningInference::new();
        let request = ReasoningRequest {
            model: "o1-mini".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };
        assert!(inference.requires_reasoning(&request).await);
    }

    #[tokio::test]
    async fn test_reasoning_detection_standard_request() {
        use gateway::routing::reasoning::{ReasoningInference, ReasoningRequest};
        use std::collections::HashMap;

        let inference = ReasoningInference::new();
        let request = ReasoningRequest {
            model: "gpt-4".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };
        assert!(!inference.requires_reasoning(&request).await);
    }

    #[tokio::test]
    async fn test_all_capabilities_in_complex_request() {
        use gateway::routing::classification::{
            ContentTypeDetector, FormatDetector, RequestFormat, StreamingExtractor, ToolDetector,
        };

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
}
