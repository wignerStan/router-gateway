#![allow(clippy::unreadable_literal, missing_docs)]
// BDD (Behavior-Driven Development) tests for request classification
//
// This module contains Cucumber-style tests that verify the behavior of
// the classification system.

#[cfg(test)]
mod classification_bdd {

    #[tokio::test]
    async fn test_bdd_classification_vision_detection() {
        use gateway::routing::classification::ContentTypeDetector;

        // Scenario: Image attachment requires vision
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

        // Scenario: Text-only content does not require vision
        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": "Hello, world!"
            }]
        });
        assert!(!ContentTypeDetector::detect_vision_required(&request));

        // Scenario: Mixed content requires vision
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
    async fn test_bdd_classification_tool_detection() {
        use gateway::routing::classification::ToolDetector;

        // Scenario: Tool definitions require tool support
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

        // Scenario: No tool definitions means no requirement
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });
        assert!(!ToolDetector::detect_tools_required(&request));

        // Scenario: Empty tool array does not require tools
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "tools": []
        });
        assert!(!ToolDetector::detect_tools_required(&request));
    }

    #[tokio::test]
    async fn test_bdd_classification_streaming_detection() {
        use gateway::routing::classification::StreamingExtractor;

        // Scenario: Explicit streaming enabled requires streaming support
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": true
        });
        assert!(StreamingExtractor::extract_streaming_preference(&request));

        // Scenario: Explicit streaming disabled does not require streaming
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": false
        });
        assert!(!StreamingExtractor::extract_streaming_preference(&request));

        // Scenario: Default behavior when streaming flag is absent
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });
        assert!(!StreamingExtractor::extract_streaming_preference(&request));
    }

    #[tokio::test]
    async fn test_bdd_classification_format_detection() {
        use gateway::routing::classification::FormatDetector;
        use gateway::routing::classification::RequestFormat;

        // Scenario: OpenAI format requests are identified by structure
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "model": "gpt-4"
        });
        assert_eq!(FormatDetector::detect(&request), RequestFormat::OpenAI);

        // Scenario: Anthropic format requests are recognized
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "system": "You are a helpful assistant",
            "model": "claude-3-opus"
        });
        assert_eq!(FormatDetector::detect(&request), RequestFormat::Anthropic);

        // Scenario: Gemini format requests are detected
        let request = serde_json::json!({
            "contents": [{"parts": [{"text": "Hello"}]}],
            "model": "gemini-pro"
        });
        assert_eq!(FormatDetector::detect(&request), RequestFormat::Gemini);

        // Scenario: Unknown format defaults to generic handling
        let request = serde_json::json!({
            "prompt": "Hello",
            "model": "unknown-model"
        });
        assert_eq!(FormatDetector::detect(&request), RequestFormat::Generic);
    }

    #[tokio::test]
    async fn test_bdd_classification_token_estimation() {
        use gateway::routing::classification::TokenEstimator;

        // Scenario: Small prompt fits standard context
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });
        let tokens = TokenEstimator::estimate(&request);
        assert!(tokens < 1000, "Small prompt should fit standard context");

        // Scenario: Large prompt requires high context capacity
        let large_text = "x".repeat(200000); // ~50000 tokens
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": large_text}]
        });
        let tokens = TokenEstimator::estimate(&request);
        assert!(tokens > 40000, "Large prompt should require high context");

        // Scenario: Total estimated tokens combines input and expected output
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
    async fn test_bdd_classification_reasoning_detection() {
        use gateway::routing::reasoning::{ReasoningInference, ReasoningRequest};
        use std::collections::HashMap;

        let inference = ReasoningInference::new();

        // Scenario: Reasoning flag explicitly enabled requires thinking support
        let request = ReasoningRequest {
            model: "gpt-4".to_string(),
            reasoning_flag: Some(true),
            max_tokens: None,
            hints: HashMap::new(),
        };
        assert!(inference.requires_reasoning(&request).await);

        // Scenario: Model family hint suggests reasoning requirement
        let request = ReasoningRequest {
            model: "o1-mini".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };
        assert!(inference.requires_reasoning(&request).await);

        // Scenario: Standard requests do not require thinking
        let request = ReasoningRequest {
            model: "gpt-4".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };
        assert!(!inference.requires_reasoning(&request).await);
    }

    #[tokio::test]
    async fn test_bdd_all_request_classification_scenarios() {
        use gateway::routing::classification::{
            ContentTypeDetector, FormatDetector, RequestFormat, StreamingExtractor, ToolDetector,
        };

        // Scenario: All capabilities detected in complex request
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
