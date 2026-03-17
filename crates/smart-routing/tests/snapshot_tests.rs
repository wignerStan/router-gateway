#![allow(clippy::unreadable_literal, missing_docs)]
// Snapshot tests for smart-routing
//
// This module contains snapshot-based tests that verify the serialized output
// of classification results, reasoning inference, and planning outputs.
// Snapshots ensure structural correctness and catch unintended changes.

#[cfg(test)]
mod snapshot_tests {

    // ============================================================
    // Classification: RequiredCapabilities
    // ============================================================

    #[test]
    fn snapshot_required_capabilities_default() {
        use smart_routing::classification::RequiredCapabilities;

        let caps = RequiredCapabilities::default();
        insta::assert_yaml_snapshot!(caps);
    }

    #[test]
    fn snapshot_required_capabilities_vision_and_tools() {
        use smart_routing::classification::RequiredCapabilities;

        let caps = RequiredCapabilities {
            vision: true,
            tools: true,
            streaming: false,
            thinking: false,
        };
        insta::assert_yaml_snapshot!(caps);
    }

    #[test]
    fn snapshot_required_capabilities_all_enabled() {
        use smart_routing::classification::RequiredCapabilities;

        let caps = RequiredCapabilities {
            vision: true,
            tools: true,
            streaming: true,
            thinking: true,
        };
        insta::assert_yaml_snapshot!(caps);
    }

    // ============================================================
    // Classification: ClassifiedRequest
    // ============================================================

    #[test]
    fn snapshot_classified_request_minimal() {
        use smart_routing::classification::{
            ClassifiedRequest, QualityPreference, RequestFormat, RequiredCapabilities,
        };

        let request = ClassifiedRequest {
            required_capabilities: RequiredCapabilities::default(),
            estimated_tokens: 0,
            format: RequestFormat::OpenAI,
            quality_preference: QualityPreference::Balanced,
        };
        insta::assert_yaml_snapshot!(request);
    }

    #[test]
    fn snapshot_classified_request_full_capabilities() {
        use smart_routing::classification::{
            ClassifiedRequest, QualityPreference, RequestFormat, RequiredCapabilities,
        };

        let request = ClassifiedRequest {
            required_capabilities: RequiredCapabilities {
                vision: true,
                tools: true,
                streaming: true,
                thinking: false,
            },
            estimated_tokens: 2500,
            format: RequestFormat::Anthropic,
            quality_preference: QualityPreference::Quality,
        };
        insta::assert_yaml_snapshot!(request);
    }

    #[test]
    fn snapshot_classified_request_speed_preference() {
        use smart_routing::classification::{
            ClassifiedRequest, QualityPreference, RequestFormat, RequiredCapabilities,
        };

        let request = ClassifiedRequest {
            required_capabilities: RequiredCapabilities {
                vision: false,
                tools: false,
                streaming: true,
                thinking: false,
            },
            estimated_tokens: 500,
            format: RequestFormat::OpenAI,
            quality_preference: QualityPreference::Speed,
        };
        insta::assert_yaml_snapshot!(request);
    }

    #[test]
    fn snapshot_classified_request_all_capabilities() {
        use smart_routing::classification::{
            ClassifiedRequest, QualityPreference, RequestFormat, RequiredCapabilities,
        };

        let request = ClassifiedRequest {
            required_capabilities: RequiredCapabilities {
                vision: true,
                tools: true,
                streaming: true,
                thinking: true,
            },
            estimated_tokens: 15_000,
            format: RequestFormat::Anthropic,
            quality_preference: QualityPreference::Quality,
        };
        insta::assert_yaml_snapshot!(request);
    }

    #[test]
    fn snapshot_classified_request_gemini() {
        use smart_routing::classification::{
            ClassifiedRequest, QualityPreference, RequestFormat, RequiredCapabilities,
        };

        let request = ClassifiedRequest {
            required_capabilities: RequiredCapabilities::default(),
            estimated_tokens: 800,
            format: RequestFormat::Gemini,
            quality_preference: QualityPreference::Balanced,
        };
        insta::assert_yaml_snapshot!(request);
    }

    // ============================================================
    // Classification: RequestFormat variants
    // ============================================================

    #[test]
    fn snapshot_request_format_openai() {
        use smart_routing::classification::RequestFormat;
        insta::assert_yaml_snapshot!(RequestFormat::OpenAI);
    }

    #[test]
    fn snapshot_request_format_anthropic() {
        use smart_routing::classification::RequestFormat;
        insta::assert_yaml_snapshot!(RequestFormat::Anthropic);
    }

    #[test]
    fn snapshot_request_format_gemini() {
        use smart_routing::classification::RequestFormat;
        insta::assert_yaml_snapshot!(RequestFormat::Gemini);
    }

    #[test]
    fn snapshot_request_format_generic() {
        use smart_routing::classification::RequestFormat;
        insta::assert_yaml_snapshot!(RequestFormat::Generic);
    }

    // ============================================================
    // Classification: QualityPreference variants
    // ============================================================

    #[test]
    fn snapshot_quality_preference_speed() {
        use smart_routing::classification::QualityPreference;
        insta::assert_yaml_snapshot!(QualityPreference::Speed);
    }

    #[test]
    fn snapshot_quality_preference_balanced() {
        use smart_routing::classification::QualityPreference;
        insta::assert_yaml_snapshot!(QualityPreference::Balanced);
    }

    #[test]
    fn snapshot_quality_preference_quality() {
        use smart_routing::classification::QualityPreference;
        insta::assert_yaml_snapshot!(QualityPreference::Quality);
    }

    // ============================================================
    // Classification: FormatDetector results
    // ============================================================

    #[test]
    fn snapshot_format_detection_openai() {
        use smart_routing::classification::FormatDetector;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "model": "gpt-4"
        });
        insta::assert_yaml_snapshot!(FormatDetector::detect(&request));
    }

    #[test]
    fn snapshot_format_detection_anthropic() {
        use smart_routing::classification::FormatDetector;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "system": "You are a helpful assistant",
            "model": "claude-3-opus"
        });
        insta::assert_yaml_snapshot!(FormatDetector::detect(&request));
    }

    #[test]
    fn snapshot_format_detection_gemini() {
        use smart_routing::classification::FormatDetector;

        let request = serde_json::json!({
            "contents": [{"parts": [{"text": "Hello"}]}],
            "model": "gemini-pro"
        });
        insta::assert_yaml_snapshot!(FormatDetector::detect(&request));
    }

    #[test]
    fn snapshot_format_detection_generic() {
        use smart_routing::classification::FormatDetector;

        let request = serde_json::json!({
            "prompt": "Hello",
            "model": "unknown-model"
        });
        insta::assert_yaml_snapshot!(FormatDetector::detect(&request));
    }

    // ============================================================
    // Classification: ContentTypeDetector results
    // ============================================================

    #[test]
    fn snapshot_vision_detection_image_url() {
        use smart_routing::classification::ContentTypeDetector;

        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "image_url",
                    "image_url": {"url": "https://example.com/image.png"}
                }]
            }]
        });
        insta::assert_yaml_snapshot!(ContentTypeDetector::detect_vision_required(&request));
    }

    #[test]
    fn snapshot_vision_detection_text_only() {
        use smart_routing::classification::ContentTypeDetector;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello, world!"}]
        });
        insta::assert_yaml_snapshot!(ContentTypeDetector::detect_vision_required(&request));
    }

    #[test]
    fn snapshot_vision_detection_mixed_content() {
        use smart_routing::classification::ContentTypeDetector;

        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "What is this?"},
                    {"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}
                ]
            }]
        });
        insta::assert_yaml_snapshot!(ContentTypeDetector::detect_vision_required(&request));
    }

    #[test]
    fn snapshot_vision_detection_anthropic_base64() {
        use smart_routing::classification::ContentTypeDetector;

        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": "iVBORw0KG..."
                    }
                }]
            }]
        });
        insta::assert_yaml_snapshot!(ContentTypeDetector::detect_vision_required(&request));
    }

    // ============================================================
    // Classification: ToolDetector results
    // ============================================================

    #[test]
    fn snapshot_tool_detection_with_tools() {
        use smart_routing::classification::ToolDetector;

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
        insta::assert_yaml_snapshot!(ToolDetector::detect_tools_required(&request));
    }

    #[test]
    fn snapshot_tool_detection_no_tools() {
        use smart_routing::classification::ToolDetector;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });
        insta::assert_yaml_snapshot!(ToolDetector::detect_tools_required(&request));
    }

    #[test]
    fn snapshot_tool_detection_legacy_functions() {
        use smart_routing::classification::ToolDetector;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "functions": [{"name": "calculate", "parameters": {}}]
        });
        insta::assert_yaml_snapshot!(ToolDetector::detect_tools_required(&request));
    }

    #[test]
    fn snapshot_tool_detection_tool_choice_auto() {
        use smart_routing::classification::ToolDetector;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "tool_choice": "auto"
        });
        insta::assert_yaml_snapshot!(ToolDetector::detect_tools_required(&request));
    }

    #[test]
    fn snapshot_tool_detection_with_tool_calls_in_messages() {
        use smart_routing::classification::ToolDetector;

        let request = serde_json::json!({
            "messages": [{
                "role": "assistant",
                "tool_calls": [{
                    "id": "call_123",
                    "type": "function",
                    "function": {"name": "get_weather", "arguments": "{}"}
                }]
            }]
        });
        insta::assert_yaml_snapshot!(ToolDetector::detect_tools_required(&request));
    }

    // ============================================================
    // Classification: StreamingExtractor results
    // ============================================================

    #[test]
    fn snapshot_streaming_detection_enabled() {
        use smart_routing::classification::StreamingExtractor;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": true
        });
        insta::assert_yaml_snapshot!(StreamingExtractor::extract_streaming_preference(&request));
    }

    #[test]
    fn snapshot_streaming_detection_disabled() {
        use smart_routing::classification::StreamingExtractor;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": false
        });
        insta::assert_yaml_snapshot!(StreamingExtractor::extract_streaming_preference(&request));
    }

    #[test]
    fn snapshot_streaming_detection_default() {
        use smart_routing::classification::StreamingExtractor;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });
        insta::assert_yaml_snapshot!(StreamingExtractor::extract_streaming_preference(&request));
    }

    // ============================================================
    // Classification: TokenEstimator results
    // ============================================================

    #[test]
    fn snapshot_token_estimation_simple_prompt() {
        use smart_routing::classification::TokenEstimator;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello, how are you?"}]
        });
        insta::assert_yaml_snapshot!(TokenEstimator::estimate(&request));
    }

    #[test]
    fn snapshot_token_estimation_with_max_tokens() {
        use smart_routing::classification::TokenEstimator;

        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "max_tokens": 1024
        });
        insta::assert_yaml_snapshot!(TokenEstimator::estimate(&request));
    }

    #[test]
    fn snapshot_token_estimation_with_system_prompt() {
        use smart_routing::classification::TokenEstimator;

        let request = serde_json::json!({
            "system": "You are a helpful assistant.",
            "messages": [{"role": "user", "content": "What is AI?"}]
        });
        insta::assert_yaml_snapshot!(TokenEstimator::estimate(&request));
    }

    #[test]
    fn snapshot_token_estimation_empty_messages() {
        use smart_routing::classification::TokenEstimator;

        let request = serde_json::json!({"messages": []});
        insta::assert_yaml_snapshot!(TokenEstimator::estimate(&request));
    }

    #[test]
    fn snapshot_token_estimation_gemini_format() {
        use smart_routing::classification::TokenEstimator;

        let request = serde_json::json!({
            "contents": [{"parts": [{"text": "What is AI?"}]}]
        });
        insta::assert_yaml_snapshot!(TokenEstimator::estimate(&request));
    }

    #[test]
    fn snapshot_token_estimation_multimodal_content() {
        use smart_routing::classification::TokenEstimator;

        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "Describe this image"},
                    {"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}
                ]
            }]
        });
        insta::assert_yaml_snapshot!(TokenEstimator::estimate(&request));
    }

    // ============================================================
    // Reasoning: Capability variants
    // ============================================================

    #[test]
    fn snapshot_reasoning_capability_none() {
        use smart_routing::reasoning::ReasoningCapability;
        insta::assert_yaml_snapshot!(ReasoningCapability::None);
    }

    #[test]
    fn snapshot_reasoning_capability_basic() {
        use smart_routing::reasoning::ReasoningCapability;
        insta::assert_yaml_snapshot!(ReasoningCapability::Basic);
    }

    #[test]
    fn snapshot_reasoning_capability_extended() {
        use smart_routing::reasoning::ReasoningCapability;
        insta::assert_yaml_snapshot!(ReasoningCapability::Extended);
    }

    #[test]
    fn snapshot_reasoning_capability_high() {
        use smart_routing::reasoning::ReasoningCapability;
        insta::assert_yaml_snapshot!(ReasoningCapability::High);
    }

    // ============================================================
    // Reasoning: Inference results for o1 series
    // ============================================================

    #[tokio::test]
    async fn snapshot_reasoning_inference_o1_preview() {
        use smart_routing::reasoning::{ReasoningInference, ReasoningRequest};
        use std::collections::HashMap;

        let inference = ReasoningInference::new();
        let request = ReasoningRequest {
            model: "o1-preview".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };
        insta::assert_yaml_snapshot!(inference.infer_capability(&request).await);
    }

    #[tokio::test]
    async fn snapshot_reasoning_inference_o1_mini() {
        use smart_routing::reasoning::{ReasoningInference, ReasoningRequest};
        use std::collections::HashMap;

        let inference = ReasoningInference::new();
        let request = ReasoningRequest {
            model: "o1-mini".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };
        insta::assert_yaml_snapshot!(inference.infer_capability(&request).await);
    }

    #[tokio::test]
    async fn snapshot_reasoning_inference_o1_pro() {
        use smart_routing::reasoning::{ReasoningInference, ReasoningRequest};
        use std::collections::HashMap;

        let inference = ReasoningInference::new();
        let request = ReasoningRequest {
            model: "o1-pro".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };
        insta::assert_yaml_snapshot!(inference.infer_capability(&request).await);
    }

    // ============================================================
    // Reasoning: Explicit flag overrides
    // ============================================================

    #[tokio::test]
    async fn snapshot_reasoning_inference_explicit_enabled() {
        use smart_routing::reasoning::{ReasoningInference, ReasoningRequest};
        use std::collections::HashMap;

        let inference = ReasoningInference::new();
        let request = ReasoningRequest {
            model: "gpt-4".to_string(),
            reasoning_flag: Some(true),
            max_tokens: None,
            hints: HashMap::new(),
        };
        insta::assert_yaml_snapshot!(inference.infer_capability(&request).await);
    }

    #[tokio::test]
    async fn snapshot_reasoning_inference_explicit_disabled() {
        use smart_routing::reasoning::{ReasoningInference, ReasoningRequest};
        use std::collections::HashMap;

        let inference = ReasoningInference::new();
        let request = ReasoningRequest {
            model: "o1-preview".to_string(),
            reasoning_flag: Some(false),
            max_tokens: None,
            hints: HashMap::new(),
        };
        insta::assert_yaml_snapshot!(inference.infer_capability(&request).await);
    }

    #[tokio::test]
    async fn snapshot_reasoning_inference_claude_thinking() {
        use smart_routing::reasoning::{ReasoningInference, ReasoningRequest};
        use std::collections::HashMap;

        let inference = ReasoningInference::new();
        let request = ReasoningRequest {
            model: "claude-3-5-thinking".to_string(),
            reasoning_flag: None,
            max_tokens: None,
            hints: HashMap::new(),
        };
        insta::assert_yaml_snapshot!(inference.infer_capability(&request).await);
    }

    #[tokio::test]
    async fn snapshot_reasoning_inference_max_tokens_hint() {
        use smart_routing::reasoning::{ReasoningInference, ReasoningRequest};
        use std::collections::HashMap;

        let inference = ReasoningInference::new();
        // Very large max_tokens hints at basic reasoning
        let request = ReasoningRequest {
            model: "gpt-4".to_string(),
            reasoning_flag: None,
            max_tokens: Some(150_000),
            hints: HashMap::new(),
        };
        insta::assert_yaml_snapshot!(inference.infer_capability(&request).await);
    }

    // ============================================================
    // Error Classification: ErrorClass from status codes
    // ============================================================

    #[test]
    fn snapshot_error_class_auth_401() {
        use smart_routing::outcome::ErrorClass;
        insta::assert_yaml_snapshot!(ErrorClass::from_status_code(401));
    }

    #[test]
    fn snapshot_error_class_auth_403() {
        use smart_routing::outcome::ErrorClass;
        insta::assert_yaml_snapshot!(ErrorClass::from_status_code(403));
    }

    #[test]
    fn snapshot_error_class_rate_limit_429() {
        use smart_routing::outcome::ErrorClass;
        insta::assert_yaml_snapshot!(ErrorClass::from_status_code(429));
    }

    #[test]
    fn snapshot_error_class_server_500() {
        use smart_routing::outcome::ErrorClass;
        insta::assert_yaml_snapshot!(ErrorClass::from_status_code(500));
    }

    #[test]
    fn snapshot_error_class_server_502() {
        use smart_routing::outcome::ErrorClass;
        insta::assert_yaml_snapshot!(ErrorClass::from_status_code(502));
    }

    #[test]
    fn snapshot_error_class_server_503() {
        use smart_routing::outcome::ErrorClass;
        insta::assert_yaml_snapshot!(ErrorClass::from_status_code(503));
    }

    #[test]
    fn snapshot_error_class_server_504() {
        use smart_routing::outcome::ErrorClass;
        insta::assert_yaml_snapshot!(ErrorClass::from_status_code(504));
    }

    #[test]
    fn snapshot_error_class_timeout_408() {
        use smart_routing::outcome::ErrorClass;
        insta::assert_yaml_snapshot!(ErrorClass::from_status_code(408));
    }

    #[test]
    fn snapshot_error_class_client_400() {
        use smart_routing::outcome::ErrorClass;
        insta::assert_yaml_snapshot!(ErrorClass::from_status_code(400));
    }

    #[test]
    fn snapshot_error_class_success_200() {
        use smart_routing::outcome::ErrorClass;
        insta::assert_yaml_snapshot!(ErrorClass::from_status_code(200));
    }

    // ============================================================
    // Capability Support: check_capability_support results
    // ============================================================

    #[test]
    fn snapshot_capability_support_supported() {
        use smart_routing::candidate::CapabilitySupport;
        insta::assert_debug_snapshot!(CapabilitySupport::Supported);
    }

    #[test]
    fn snapshot_capability_support_missing_vision() {
        use smart_routing::candidate::CapabilitySupport;
        insta::assert_debug_snapshot!(CapabilitySupport::Unsupported {
            missing_vision: true,
            missing_tools: false,
            missing_streaming: false,
            missing_thinking: false,
        });
    }

    #[test]
    fn snapshot_capability_support_missing_tools_and_thinking() {
        use smart_routing::candidate::CapabilitySupport;
        insta::assert_debug_snapshot!(CapabilitySupport::Unsupported {
            missing_vision: false,
            missing_tools: true,
            missing_streaming: false,
            missing_thinking: true,
        });
    }

    #[test]
    fn snapshot_capability_support_all_missing() {
        use smart_routing::candidate::CapabilitySupport;
        insta::assert_debug_snapshot!(CapabilitySupport::Unsupported {
            missing_vision: true,
            missing_tools: true,
            missing_streaming: true,
            missing_thinking: true,
        });
    }

    #[test]
    fn snapshot_capability_support_description_supported() {
        use smart_routing::candidate::CapabilitySupport;
        insta::assert_yaml_snapshot!(CapabilitySupport::Supported.missing_description());
    }

    #[test]
    fn snapshot_capability_support_description_missing_vision() {
        use smart_routing::candidate::CapabilitySupport;
        let support = CapabilitySupport::Unsupported {
            missing_vision: true,
            missing_tools: false,
            missing_streaming: false,
            missing_thinking: false,
        };
        insta::assert_yaml_snapshot!(support.missing_description());
    }

    #[test]
    fn snapshot_capability_support_description_missing_multiple() {
        use smart_routing::candidate::CapabilitySupport;
        let support = CapabilitySupport::Unsupported {
            missing_vision: true,
            missing_tools: true,
            missing_streaming: false,
            missing_thinking: true,
        };
        insta::assert_yaml_snapshot!(support.missing_description());
    }

    // ============================================================
    // Token Fit Status: variants
    // ============================================================

    #[test]
    fn snapshot_token_fit_fits() {
        use smart_routing::candidate::TokenFitStatus;
        insta::assert_debug_snapshot!(TokenFitStatus::Fits);
    }

    #[test]
    fn snapshot_token_fit_exceeds() {
        use smart_routing::candidate::TokenFitStatus;
        insta::assert_debug_snapshot!(TokenFitStatus::Exceeds);
    }

    #[test]
    fn snapshot_token_fit_unknown() {
        use smart_routing::candidate::TokenFitStatus;
        insta::assert_debug_snapshot!(TokenFitStatus::Unknown);
    }

    // ============================================================
    // Filter Result: variants
    // ============================================================

    #[test]
    fn snapshot_filter_result_accepted() {
        use smart_routing::filtering::FilterResult;
        insta::assert_debug_snapshot!(FilterResult::Accepted);
    }

    #[test]
    fn snapshot_filter_result_rejected_capability() {
        use smart_routing::filtering::FilterResult;
        insta::assert_debug_snapshot!(FilterResult::Rejected {
            reason: "capability mismatch: missing vision".to_string(),
        });
    }

    #[test]
    fn snapshot_filter_result_rejected_context() {
        use smart_routing::filtering::FilterResult;
        insta::assert_debug_snapshot!(FilterResult::Rejected {
            reason: "context overflow: 200000 tokens exceeds model gpt-4 context window of 128000"
                .to_string(),
        });
    }

    #[test]
    fn snapshot_filter_result_rejected_provider() {
        use smart_routing::filtering::FilterResult;
        insta::assert_debug_snapshot!(FilterResult::Rejected {
            reason: "provider disabled: blocked-provider".to_string(),
        });
    }

    // ============================================================
    // Health Status: variants
    // ============================================================

    #[test]
    fn snapshot_health_status_healthy() {
        use smart_routing::health::HealthStatus;
        insta::assert_debug_snapshot!(HealthStatus::Healthy);
    }

    #[test]
    fn snapshot_health_status_degraded() {
        use smart_routing::health::HealthStatus;
        insta::assert_debug_snapshot!(HealthStatus::Degraded);
    }

    #[test]
    fn snapshot_health_status_unhealthy() {
        use smart_routing::health::HealthStatus;
        insta::assert_debug_snapshot!(HealthStatus::Unhealthy);
    }

    // ============================================================
    // Fallback Config: defaults
    // ============================================================

    #[test]
    fn snapshot_fallback_config_defaults() {
        use smart_routing::fallback::FallbackConfig;
        insta::assert_debug_snapshot!(FallbackConfig::default());
    }

    // ============================================================
    // Execution Outcome: success/failure/timeout/network
    // ============================================================

    #[test]
    fn snapshot_execution_outcome_success() {
        use smart_routing::outcome::ExecutionOutcome;

        let outcome = ExecutionOutcome::success("route-1".to_string(), 150.0, 100, 50, 200);
        insta::assert_yaml_snapshot!(outcome, {
            ".timestamp" => "[timestamp]"
        });
    }

    #[test]
    fn snapshot_execution_outcome_failure() {
        use smart_routing::outcome::ExecutionOutcome;

        let outcome = ExecutionOutcome::failure("route-1".to_string(), 200.0, 500, false, None);
        insta::assert_yaml_snapshot!(outcome, {
            ".timestamp" => "[timestamp]"
        });
    }

    #[test]
    fn snapshot_execution_outcome_timeout() {
        use smart_routing::outcome::ExecutionOutcome;

        let outcome = ExecutionOutcome::timeout("route-1".to_string(), 30_000.0);
        insta::assert_yaml_snapshot!(outcome, {
            ".timestamp" => "[timestamp]"
        });
    }

    #[test]
    fn snapshot_execution_outcome_network_error() {
        use smart_routing::outcome::ExecutionOutcome;

        let outcome = ExecutionOutcome::network_error("route-1".to_string(), 5_000.0);
        insta::assert_yaml_snapshot!(outcome, {
            ".timestamp" => "[timestamp]"
        });
    }

    #[test]
    fn snapshot_execution_outcome_with_fallback() {
        use smart_routing::outcome::ExecutionOutcome;

        let outcome = ExecutionOutcome::failure(
            "route-fallback".to_string(),
            300.0,
            500,
            true,
            Some("route-original".to_string()),
        );
        insta::assert_yaml_snapshot!(outcome, {
            ".timestamp" => "[timestamp]"
        });
    }
}
