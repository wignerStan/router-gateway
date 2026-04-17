#![allow(
    clippy::unreadable_literal,
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used
)]
// Integration tests for request classification
//
// Unit-level integration tests covering vision detection, tool detection,
// streaming extraction, format detection, token estimation, and reasoning.
// Behavioral BDD coverage lives in tests/bdd/classification.rs (cucumber).

#[cfg(test)]
mod classification {
    use gateway::routing::classification::{
        ContentTypeDetector, FormatDetector, RequestFormat, StreamingExtractor, TokenEstimator,
        ToolDetector,
    };
    use gateway::routing::reasoning::{ReasoningInference, ReasoningRequest};
    use rstest::rstest;
    use std::collections::HashMap;

    #[rstest]
    #[case::image_url(serde_json::json!({"messages": [{"role": "user", "content": [{"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}]}]}), true)]
    #[case::text_only(serde_json::json!({"messages": [{"role": "user", "content": "Hello, world!"}]}), false)]
    #[case::mixed_content(serde_json::json!({"messages": [{"role": "user", "content": [{"type": "text", "text": "What is this?"}, {"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}]}]}), true)]
    #[tokio::test]
    async fn test_vision_detection(#[case] request: serde_json::Value, #[case] expected: bool) {
        assert_eq!(
            ContentTypeDetector::detect_vision_required(&request),
            expected
        );
    }

    #[rstest]
    #[case::with_tools(serde_json::json!({"messages": [{"role": "user", "content": "What's the weather?"}], "tools": [{"type": "function", "function": {"name": "get_weather", "parameters": {"type": "object"}}}]}), true)]
    #[case::without_tools(serde_json::json!({"messages": [{"role": "user", "content": "Hello"}]}), false)]
    #[case::empty_array(serde_json::json!({"messages": [{"role": "user", "content": "Hello"}], "tools": []}), false)]
    #[tokio::test]
    async fn test_tool_detection(#[case] request: serde_json::Value, #[case] expected: bool) {
        assert_eq!(ToolDetector::detect_tools_required(&request), expected);
    }

    #[rstest]
    #[case::enabled(serde_json::json!({"messages": [{"role": "user", "content": "Hello"}], "stream": true}), true)]
    #[case::disabled(serde_json::json!({"messages": [{"role": "user", "content": "Hello"}], "stream": false}), false)]
    #[case::absent(serde_json::json!({"messages": [{"role": "user", "content": "Hello"}]}), false)]
    #[tokio::test]
    async fn test_streaming_detection(#[case] request: serde_json::Value, #[case] expected: bool) {
        assert_eq!(
            StreamingExtractor::extract_streaming_preference(&request),
            expected
        );
    }

    #[rstest]
    #[case::openai(serde_json::json!({"messages": [{"role": "user", "content": "Hello"}], "model": "gpt-4"}), RequestFormat::OpenAI)]
    #[case::anthropic(serde_json::json!({"messages": [{"role": "user", "content": "Hello"}], "system": "You are a helpful assistant", "model": "claude-3-opus"}), RequestFormat::Anthropic)]
    #[case::gemini(serde_json::json!({"contents": [{"parts": [{"text": "Hello"}]}], "model": "gemini-pro"}), RequestFormat::Gemini)]
    #[case::unknown(serde_json::json!({"prompt": "Hello", "model": "unknown-model"}), RequestFormat::Generic)]
    #[tokio::test]
    async fn test_format_detection(
        #[case] request: serde_json::Value,
        #[case] expected: RequestFormat,
    ) {
        assert_eq!(FormatDetector::detect(&request), expected);
    }

    #[tokio::test]
    async fn test_token_estimation_small_prompt() {
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });
        let tokens = TokenEstimator::estimate(&request);
        assert!(tokens < 1000, "Small prompt should fit standard context");
    }

    #[tokio::test]
    async fn test_token_estimation_large_prompt() {
        let large_text = "x".repeat(200000); // ~50000 tokens
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": large_text}]
        });
        let tokens = TokenEstimator::estimate(&request);
        assert!(tokens > 40000, "Large prompt should require high context");
    }

    #[tokio::test]
    async fn test_token_estimation_combined_input_output() {
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

    #[rstest]
    #[case::explicit_flag(ReasoningRequest { model: "gpt-4".to_string(), reasoning_flag: Some(true), max_tokens: None, hints: HashMap::new() }, true)]
    #[case::model_family(ReasoningRequest { model: "o1-mini".to_string(), reasoning_flag: None, max_tokens: None, hints: HashMap::new() }, true)]
    #[case::standard_request(ReasoningRequest { model: "gpt-4".to_string(), reasoning_flag: None, max_tokens: None, hints: HashMap::new() }, false)]
    #[tokio::test]
    async fn test_reasoning_detection(#[case] request: ReasoningRequest, #[case] expected: bool) {
        let inference = ReasoningInference::new();
        assert_eq!(inference.requires_reasoning(&request).await, expected);
    }

    #[tokio::test]
    async fn test_all_capabilities_in_complex_request() {
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
