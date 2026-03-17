// Request classification step definitions
// (docs/features/request-classification/request-classification.feature)

#![allow(
    clippy::unreadable_literal,
    missing_docs,
    clippy::expect_used,
    clippy::trivial_regex,
    clippy::unused_async,
    clippy::needless_pass_by_ref_mut,
    clippy::unwrap_used,
    clippy::panic,
    clippy::used_underscore_binding,
    clippy::float_cmp
)]

use cucumber::{given, then, when};
use serde_json::json;
use smart_routing::classification::{
    ContentTypeDetector, FormatDetector, RequestFormat, StreamingExtractor, TokenEstimator,
    ToolDetector,
};
use smart_routing::reasoning::{ReasoningInference, ReasoningRequest};
use std::collections::HashMap;

use super::super::{BddWorld, ClassificationResult};

// ============================================================================
// REQUEST CLASSIFICATION STEP DEFINITIONS
// (docs/features/request-classification/request-classification.feature)
// ============================================================================

// -- Given: content type steps --

#[given("a chat request containing an image attachment")]
async fn given_image_attachment(world: &mut BddWorld) {
    world.current_request = Some(json!({
        "messages": [{
            "role": "user",
            "content": [{
                "type": "image_url",
                "image_url": {"url": "https://example.com/image.png"}
            }]
        }]
    }));
}

#[given("a chat request containing only text content")]
async fn given_text_only(world: &mut BddWorld) {
    world.current_request = Some(json!({
        "messages": [{"role": "user", "content": "Hello, world!"}]
    }));
}

#[given("a request with both text and image content")]
async fn given_mixed_content(world: &mut BddWorld) {
    world.current_request = Some(json!({
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": "What is this?"},
                {"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}
            ]
        }]
    }));
}

// -- Given: tool steps --

#[given("a request containing tool function definitions")]
async fn given_with_tools(world: &mut BddWorld) {
    world.current_request = Some(json!({
        "messages": [{"role": "user", "content": "What's the weather?"}],
        "tools": [{
            "type": "function",
            "function": {
                "name": "get_weather",
                "parameters": {"type": "object"}
            }
        }]
    }));
}

#[given("a request with no tool definitions")]
async fn given_no_tools(world: &mut BddWorld) {
    world.current_request = Some(json!({
        "messages": [{"role": "user", "content": "Hello"}]
    }));
}

#[given("a request with an empty tool list")]
async fn given_empty_tools(world: &mut BddWorld) {
    world.current_request = Some(json!({
        "messages": [{"role": "user", "content": "Hello"}],
        "tools": []
    }));
}

// -- Given: streaming steps --

#[given("a request with streaming enabled")]
async fn given_streaming_enabled(world: &mut BddWorld) {
    world.current_request = Some(json!({
        "messages": [{"role": "user", "content": "Hello"}],
        "stream": true
    }));
}

#[given("a request with streaming disabled")]
async fn given_streaming_disabled(world: &mut BddWorld) {
    world.current_request = Some(json!({
        "messages": [{"role": "user", "content": "Hello"}],
        "stream": false
    }));
}

#[given("a request without a streaming parameter")]
async fn given_no_streaming_param(world: &mut BddWorld) {
    world.current_request = Some(json!({
        "messages": [{"role": "user", "content": "Hello"}]
    }));
}

// -- Given: reasoning steps --

#[given("a request with reasoning enabled in parameters")]
async fn given_reasoning_enabled(world: &mut BddWorld) {
    world.reasoning_request = Some(ReasoningRequest {
        model: "gpt-4".to_string(),
        reasoning_flag: Some(true),
        max_tokens: None,
        hints: HashMap::new(),
    });
    // Also set a JSON request for classification
    world.current_request = Some(json!({
        "messages": [{"role": "user", "content": "Think step by step"}],
        "reasoning_effort": "high"
    }));
}

#[given("a request targeting a reasoning-optimized model family")]
async fn given_reasoning_model_family(world: &mut BddWorld) {
    world.reasoning_request = Some(ReasoningRequest {
        model: "o1-mini".to_string(),
        reasoning_flag: None,
        max_tokens: None,
        hints: HashMap::new(),
    });
    world.current_request = Some(json!({
        "messages": [{"role": "user", "content": "Solve this problem"}],
        "model": "o1-mini"
    }));
}

#[given("a request with no reasoning indicators")]
async fn given_no_reasoning(world: &mut BddWorld) {
    world.reasoning_request = Some(ReasoningRequest {
        model: "gpt-4".to_string(),
        reasoning_flag: None,
        max_tokens: None,
        hints: HashMap::new(),
    });
    world.current_request = Some(json!({
        "messages": [{"role": "user", "content": "Hello"}]
    }));
}

// -- Given: format steps --

#[given("a request with OpenAI-compatible message format")]
async fn given_openai_format(world: &mut BddWorld) {
    world.current_request = Some(json!({
        "messages": [{"role": "user", "content": "Hello"}],
        "model": "gpt-4"
    }));
}

#[given("a request with Anthropic message format")]
async fn given_anthropic_format(world: &mut BddWorld) {
    world.current_request = Some(json!({
        "messages": [{"role": "user", "content": "Hello"}],
        "system": "You are a helpful assistant",
        "model": "claude-3-opus"
    }));
}

#[given("a request with Gemini message structure")]
async fn given_gemini_format(world: &mut BddWorld) {
    world.current_request = Some(json!({
        "contents": [{"parts": [{"text": "Hello"}]}],
        "model": "gemini-pro"
    }));
}

#[given("a request with unrecognized message structure")]
async fn given_unknown_format(world: &mut BddWorld) {
    world.current_request = Some(json!({
        "prompt": "Hello",
        "model": "unknown-model"
    }));
}

// -- Given: token estimation steps --

#[given(regex = r"a request with a prompt containing (\d+) tokens")]
async fn given_prompt_with_tokens(world: &mut BddWorld, tokens: u64) {
    // ~4 chars per token
    let chars = tokens * 4;
    world.current_request = Some(json!({
        "messages": [{"role": "user", "content": "x".repeat(chars as usize)}]
    }));
}

#[given(regex = r"a request with (\d+) input tokens")]
async fn given_input_tokens(world: &mut BddWorld, tokens: u64) {
    let chars = tokens * 4;
    world.current_request = Some(json!({
        "messages": [{"role": "user", "content": "x".repeat(chars as usize)}]
    }));
}

#[given(regex = r"an expected output of (\d+) tokens")]
async fn given_expected_output(world: &mut BddWorld, tokens: u64) {
    world.expected_output_tokens = tokens as u32;
}

// -- When: classification --

#[when("the request is classified")]
async fn when_classified(world: &mut BddWorld) {
    let request = world
        .current_request
        .as_ref()
        .expect("request must be set before classification");

    // Run all detectors
    let vision_required = ContentTypeDetector::detect_vision_required(request);
    let tools_required = ToolDetector::detect_tools_required(request);
    let streaming_required = StreamingExtractor::extract_streaming_preference(request);
    let format = FormatDetector::detect(request);

    // Token estimation — total includes input + output
    let estimated_tokens = TokenEstimator::estimate(request);

    // Estimate input tokens separately (content only, no output default)
    let estimated_input_tokens = {
        let mut total_chars = 0u64;
        if let Some(messages) = request.get("messages").and_then(|m| m.as_array()) {
            for msg in messages {
                if let Some(content) = msg.get("content") {
                    if let Some(s) = content.as_str() {
                        total_chars += s.len() as u64;
                    } else if let Some(arr) = content.as_array() {
                        for part in arr {
                            if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                total_chars += text.len() as u64;
                            }
                        }
                    }
                }
            }
        } else if let Some(contents) = request.get("contents").and_then(|c| c.as_array()) {
            for content in contents {
                if let Some(parts) = content.get("parts").and_then(|p| p.as_array()) {
                    for part in parts {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            total_chars += text.len() as u64;
                        }
                    }
                }
            }
        } else if let Some(prompt) = request.get("prompt").and_then(|p| p.as_str()) {
            total_chars += prompt.len() as u64;
        }
        if let Some(system) = request.get("system").and_then(|s| s.as_str()) {
            total_chars += system.len() as u64;
        }
        ((total_chars as f64) / 4.0).ceil() as u32
    };

    let estimated_output_tokens = estimated_tokens.saturating_sub(estimated_input_tokens);

    // Reasoning detection
    let thinking_required = if let Some(ref reasoning_req) = world.reasoning_request {
        let inference = ReasoningInference::new();
        inference.requires_reasoning(reasoning_req).await
    } else {
        false
    };

    world.classification_result = Some(ClassificationResult {
        vision_required,
        tools_required,
        streaming_required,
        thinking_required,
        format,
        estimated_tokens,
        estimated_input_tokens,
        estimated_output_tokens,
    });
}

// -- Then: classification --

#[then("vision capability should be required")]
async fn then_vision_required(world: &mut BddWorld) {
    let result = world
        .classification_result
        .as_ref()
        .expect("classification must be run first");
    assert!(
        result.vision_required,
        "vision capability should be required"
    );
}

#[then("vision capability should not be required")]
async fn then_vision_not_required(world: &mut BddWorld) {
    let result = world
        .classification_result
        .as_ref()
        .expect("classification must be run first");
    assert!(
        !result.vision_required,
        "vision capability should NOT be required"
    );
}

#[then("tool capability should be required")]
async fn then_tools_required(world: &mut BddWorld) {
    let result = world
        .classification_result
        .as_ref()
        .expect("classification must be run first");
    assert!(result.tools_required, "tool capability should be required");
}

#[then("tool capability should not be required")]
async fn then_tools_not_required(world: &mut BddWorld) {
    let result = world
        .classification_result
        .as_ref()
        .expect("classification must be run first");
    assert!(
        !result.tools_required,
        "tool capability should NOT be required"
    );
}

#[then("streaming capability should be required")]
async fn then_streaming_required(world: &mut BddWorld) {
    let result = world
        .classification_result
        .as_ref()
        .expect("classification must be run first");
    assert!(
        result.streaming_required,
        "streaming capability should be required"
    );
}

#[then("streaming capability should not be required")]
async fn then_streaming_not_required(world: &mut BddWorld) {
    let result = world
        .classification_result
        .as_ref()
        .expect("classification must be run first");
    assert!(
        !result.streaming_required,
        "streaming capability should NOT be required"
    );
}

#[then("thinking capability should be required")]
async fn then_thinking_required(world: &mut BddWorld) {
    let result = world
        .classification_result
        .as_ref()
        .expect("classification must be run first");
    assert!(
        result.thinking_required,
        "thinking capability should be required"
    );
}

#[then("thinking capability should not be required")]
async fn then_thinking_not_required(world: &mut BddWorld) {
    let result = world
        .classification_result
        .as_ref()
        .expect("classification must be run first");
    assert!(
        !result.thinking_required,
        "thinking capability should NOT be required"
    );
}

#[then(regex = r"the format should be identified as (.+)")]
async fn then_format_identified(world: &mut BddWorld, format_name: String) {
    let result = world
        .classification_result
        .as_ref()
        .expect("classification must be run first");
    let expected = match format_name.as_str() {
        "OpenAI" => RequestFormat::OpenAI,
        "Anthropic" => RequestFormat::Anthropic,
        "Gemini" => RequestFormat::Gemini,
        "generic" => RequestFormat::Generic,
        other => panic!("unknown format: {other}"),
    };
    assert_eq!(result.format, expected, "format should be {format_name}");
}

#[then(regex = r"the estimated input tokens should be (\d+)")]
async fn then_estimated_input_tokens(world: &mut BddWorld, expected_tokens: u64) {
    let result = world
        .classification_result
        .as_ref()
        .expect("classification must be run first");
    // Allow ±20% tolerance due to character-based estimation
    let tolerance = (expected_tokens as f64 * 0.20) as u32;
    let low = expected_tokens.saturating_sub(u64::from(tolerance)) as u32;
    let high = expected_tokens as u32 + tolerance;
    assert!(
        result.estimated_input_tokens >= low && result.estimated_input_tokens <= high,
        "estimated input tokens {} should be ~{expected_tokens} (±20%)",
        result.estimated_input_tokens,
    );
}

#[then("a large context window should be required")]
async fn then_large_context(world: &mut BddWorld) {
    let result = world
        .classification_result
        .as_ref()
        .expect("classification must be run first");
    assert!(
        result.estimated_input_tokens > 40000,
        "large context window should be required (input tokens: {})",
        result.estimated_input_tokens,
    );
}

#[then("a standard context window should suffice")]
async fn then_standard_context(world: &mut BddWorld) {
    let result = world
        .classification_result
        .as_ref()
        .expect("classification must be run first");
    assert!(
        result.estimated_input_tokens < 4000,
        "standard context window should suffice (input tokens: {})",
        result.estimated_input_tokens,
    );
}

#[then("the total estimated tokens should be 1500")]
async fn then_total_tokens_1500(world: &mut BddWorld) {
    let result = world
        .classification_result
        .as_ref()
        .expect("classification must be run first");
    // Total = input (~1000) + output (500 from max_tokens field) + overhead
    // TokenEstimator includes max_tokens as output, so total is ~1500
    let expected = result.estimated_input_tokens + world.expected_output_tokens;
    // Allow ±20% tolerance
    assert!(
        result.estimated_tokens > (f64::from(expected) * 0.8) as u32
            && result.estimated_tokens < (f64::from(expected) * 1.2) as u32,
        "total estimated tokens should be ~{expected} (got {})",
        result.estimated_tokens,
    );
}
