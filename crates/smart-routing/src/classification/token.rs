#![allow(clippy::unreadable_literal)]
use serde_json::Value;

/// Token estimator for LLM requests
///
/// Estimates token counts from request content for context window sizing.
/// Uses a simple heuristic (~4 characters per token) which is approximate
/// but sufficient for routing decisions.
pub struct TokenEstimator;

impl TokenEstimator {
    /// Average characters per token (English text)
    const CHARS_PER_TOKEN: f64 = 4.0;

    /// Default expected output tokens when not specified
    const DEFAULT_OUTPUT_TOKENS: u32 = 512;

    /// Estimate total tokens for a request
    ///
    /// Combines input tokens (from content) with expected output tokens.
    ///
    /// # Arguments
    /// * `request` - The request JSON to estimate tokens from
    ///
    /// # Returns
    /// Estimated total tokens (input + expected output)
    #[must_use]
    pub fn estimate(request: &Value) -> u32 {
        let input_tokens = Self::estimate_input(request);
        let output_tokens = Self::estimate_output(request);

        input_tokens.saturating_add(output_tokens)
    }

    /// Estimate input tokens from request content
    fn estimate_input(request: &Value) -> u32 {
        let mut total_chars = 0u64;

        // Extract text from various message formats
        if let Some(messages) = request.get("messages").and_then(|m| m.as_array()) {
            // OpenAI/Anthropic format
            for msg in messages {
                total_chars += Self::extract_chars_from_message(msg);
            }
        } else if let Some(contents) = request.get("contents").and_then(|c| c.as_array()) {
            // Gemini format
            for content in contents {
                total_chars += Self::extract_chars_from_gemini_content(content);
            }
        } else if let Some(prompt) = request.get("prompt").and_then(|p| p.as_str()) {
            // Simple prompt format
            total_chars += prompt.len() as u64;
        }

        // Also check system prompt
        if let Some(system) = request.get("system").and_then(|s| s.as_str()) {
            total_chars += system.len() as u64;
        }

        // Convert characters to tokens (round up)
        ((total_chars as f64) / Self::CHARS_PER_TOKEN).ceil() as u32
    }

    /// Extract characters from a message object
    fn extract_chars_from_message(msg: &Value) -> u64 {
        if let Some(content) = msg.get("content") {
            // Content can be a string or an array (for multimodal)
            if let Some(text) = content.as_str() {
                return text.len() as u64;
            } else if let Some(parts) = content.as_array() {
                let mut total = 0u64;
                for part in parts {
                    if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                        total += text.len() as u64;
                    }
                    // Note: We ignore image URLs for token estimation
                    // as images are counted differently
                }
                return total;
            }
        }
        0
    }

    /// Extract characters from Gemini content object
    fn extract_chars_from_gemini_content(content: &Value) -> u64 {
        if let Some(parts) = content.get("parts").and_then(|p| p.as_array()) {
            let mut total = 0u64;
            for part in parts {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    total += text.len() as u64;
                }
            }
            return total;
        }
        0
    }

    /// Estimate expected output tokens
    fn estimate_output(request: &Value) -> u32 {
        // Check if max_tokens is specified
        if let Some(max_tokens) = request
            .get("max_tokens")
            .or_else(|| request.get("max_completion_tokens"))
            .and_then(serde_json::Value::as_u64)
        {
            return max_tokens as u32;
        }

        // Use default
        Self::DEFAULT_OUTPUT_TOKENS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_estimate_small_prompt_standard_context() {
        let request = json!({
            "messages": [
                {"role": "user", "content": "Hello, how are you?"}
            ]
        });

        let tokens = TokenEstimator::estimate(&request);
        // Input: ~25 chars / 4 = ~7 tokens + 512 default output
        assert!(
            tokens > 500 && tokens < 550,
            "Expected ~519 tokens, got {tokens}"
        );
    }

    #[test]
    fn test_estimate_large_prompt_high_context() {
        let large_text = "x".repeat(10000); // 10k characters
        let request = json!({
            "messages": [
                {"role": "user", "content": large_text}
            ]
        });

        let tokens = TokenEstimator::estimate(&request);
        // Input: 10000 / 4 = 2500 tokens + 512 default output
        assert!(
            tokens > 3000 && tokens < 3100,
            "Expected ~3012 tokens, got {tokens}"
        );
    }

    #[test]
    fn test_estimate_with_max_tokens() {
        let request = json!({
            "messages": [
                {"role": "user", "content": "Hello"}
            ],
            "max_tokens": 1024
        });

        let tokens = TokenEstimator::estimate(&request);
        // Input: ~5 chars / 4 = ~2 tokens + 1024 max output
        assert!(
            tokens > 1020 && tokens < 1030,
            "Expected ~1026 tokens, got {tokens}"
        );
    }

    #[test]
    fn test_estimate_with_system_prompt() {
        let request = json!({
            "system": "You are a helpful assistant that provides detailed answers.",
            "messages": [
                {"role": "user", "content": "What is the capital of France?"}
            ]
        });

        let tokens = TokenEstimator::estimate(&request);
        // System: ~60 chars / 4 = 15 tokens
        // User: ~35 chars / 4 = 9 tokens
        // Total input: ~24 tokens + 512 default output
        assert!(
            tokens > 530 && tokens < 540,
            "Expected ~536 tokens, got {tokens}"
        );
    }

    #[test]
    fn test_estimate_multimodal_content() {
        let request = json!({
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "Describe this image"},
                        {"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}
                    ]
                }
            ]
        });

        let tokens = TokenEstimator::estimate(&request);
        // Only text is counted, image URL is ignored
        // Input: ~20 chars / 4 = ~5 tokens + 512 default output
        assert!(
            tokens > 515 && tokens < 525,
            "Expected ~517 tokens, got {tokens}"
        );
    }

    #[test]
    fn test_estimate_gemini_format() {
        let request = json!({
            "contents": [
                {"parts": [{"text": "What is AI?"}]}
            ]
        });

        let tokens = TokenEstimator::estimate(&request);
        // Input: ~11 chars / 4 = ~3 tokens + 512 default output
        assert!(
            tokens > 510 && tokens < 520,
            "Expected ~515 tokens, got {tokens}"
        );
    }

    #[test]
    fn test_estimate_simple_prompt_format() {
        let request = json!({
            "prompt": "Once upon a time"
        });

        let tokens = TokenEstimator::estimate(&request);
        // Input: ~16 chars / 4 = ~4 tokens + 512 default output
        assert!(
            tokens > 510 && tokens < 520,
            "Expected ~516 tokens, got {tokens}"
        );
    }

    #[test]
    fn test_estimate_total_tokens_calculation() {
        let request = json!({
            "messages": [
                {"role": "user", "content": "x".repeat(2000)}  // 2000 chars
            ],
            "max_tokens": 2000
        });

        let tokens = TokenEstimator::estimate(&request);
        // Input: 2000 / 4 = 500 tokens
        // Output: 2000 tokens
        // Total: 2500 tokens
        assert!(
            tokens > 2490 && tokens < 2510,
            "Expected ~2500 tokens, got {tokens}"
        );
    }

    // ============================================================
    // Edge Case Tests for Token Estimation
    // ============================================================

    #[test]
    fn test_estimate_empty_messages() {
        let request = json!({
            "messages": []
        });

        let tokens = TokenEstimator::estimate(&request);
        // Empty messages = 0 input tokens + 512 default output
        assert_eq!(
            tokens, 512,
            "Empty messages should only have default output tokens"
        );
    }

    #[test]
    fn test_estimate_empty_content() {
        let request = json!({
            "messages": [
                {"role": "user", "content": ""}
            ]
        });

        let tokens = TokenEstimator::estimate(&request);
        // Empty content = 0 input tokens + 512 default output
        assert_eq!(
            tokens, 512,
            "Empty content should only have default output tokens"
        );
    }

    #[test]
    fn test_estimate_very_large_content() {
        // Create content with 1 million characters
        let large_text = "x".repeat(1_000_000);
        let request = json!({
            "messages": [
                {"role": "user", "content": large_text}
            ]
        });

        let tokens = TokenEstimator::estimate(&request);
        // Input: 1_000_000 / 4 = 250,000 tokens + 512 output
        // Should handle without overflow (using saturating_add)
        assert!(
            tokens > 250_000,
            "Should handle very large content without overflow"
        );
        assert!(tokens < 300_000, "Token count should be reasonable");
    }

    #[test]
    fn test_estimate_multimodal_with_invalid_image_url() {
        // Test with malformed image_url (missing url field)
        let request = json!({
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "Describe this"},
                        {"type": "image_url", "image_url": {}},  // Missing url
                        {"type": "image_url"}  // Missing image_url entirely
                    ]
                }
            ]
        });

        let tokens = TokenEstimator::estimate(&request);
        // Should not panic, only count text portion
        // Input: ~15 chars / 4 = ~4 tokens + 512 output
        assert!(
            tokens > 510 && tokens < 520,
            "Should handle malformed image_url gracefully, got {tokens}"
        );
    }

    #[test]
    fn test_estimate_with_empty_tools_array() {
        // Tools themselves don't add to token estimation currently
        // but this tests that empty arrays don't cause issues
        let request = json!({
            "messages": [
                {"role": "user", "content": "Hello"}
            ],
            "tools": []
        });

        let tokens = TokenEstimator::estimate(&request);
        // Should handle empty tools without issue
        assert!(
            tokens > 510 && tokens < 520,
            "Should handle empty tools array"
        );
    }

    #[test]
    fn test_estimate_with_null_content() {
        let request = json!({
            "messages": [
                {"role": "user", "content": null}
            ]
        });

        let tokens = TokenEstimator::estimate(&request);
        // Should handle null content gracefully
        assert_eq!(
            tokens, 512,
            "Null content should only have default output tokens"
        );
    }

    #[test]
    fn test_estimate_with_missing_content_field() {
        let request = json!({
            "messages": [
                {"role": "user"}
            ]
        });

        let tokens = TokenEstimator::estimate(&request);
        // Should handle missing content gracefully
        assert_eq!(
            tokens, 512,
            "Missing content should only have default output tokens"
        );
    }

    #[test]
    fn test_estimate_with_max_completion_tokens() {
        let request = json!({
            "messages": [
                {"role": "user", "content": "Hello"}
            ],
            "max_completion_tokens": 2048
        });

        let tokens = TokenEstimator::estimate(&request);
        // Input: ~5 chars / 4 = ~2 tokens + 2048 output
        assert!(
            tokens > 2045 && tokens < 2055,
            "Should use max_completion_tokens"
        );
    }

    #[test]
    fn test_estimate_saturating_add_prevents_overflow() {
        // Test that saturating_add prevents overflow
        let huge_text = "x".repeat(100_000_000); // 100M characters
        let request = json!({
            "messages": [
                {"role": "user", "content": huge_text}
            ],
            "max_tokens": 2000000000  // Large value within i32 range
        });

        let tokens = TokenEstimator::estimate(&request);
        // Should not panic, should saturate if needed
        assert!(tokens > 0, "Should handle extreme values");
    }

    #[test]
    fn test_estimate_with_unicode_content() {
        // Unicode characters may be more than 1 byte
        let request = json!({
            "messages": [
                {"role": "user", "content": "你好世界こんにちは세상"}
            ]
        });

        let tokens = TokenEstimator::estimate(&request);
        // Should handle unicode - counts chars, not bytes
        // ~11 unicode chars / 4 = ~3 tokens + JSON overhead + 512 output
        assert!(
            tokens > 510 && tokens < 530,
            "Should handle unicode content, got {tokens}"
        );
    }

    #[test]
    fn test_estimate_mixed_content_parts() {
        let request = json!({
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "First part"},
                        {"type": "image_url", "image_url": {"url": "https://example.com/img.png"}},
                        {"type": "text", "text": "Second part"}
                    ]
                }
            ]
        });

        let tokens = TokenEstimator::estimate(&request);
        // Only text parts counted: "First part" + "Second part" = ~21 chars
        // ~21 / 4 = ~6 tokens + 512 output
        assert!(
            tokens > 515 && tokens < 525,
            "Should count multiple text parts, got {tokens}"
        );
    }
}
