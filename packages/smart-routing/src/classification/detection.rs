use serde_json::Value;

/// Detector for image content in requests
pub struct ContentTypeDetector;

impl ContentTypeDetector {
    /// Detect if the request contains image content requiring vision capability
    ///
    /// Scans through message content for image attachments, image URLs,
    /// or image data URIs. Returns true if any image content is found.
    ///
    /// # Scenarios
    /// - Image attachment present → requires vision
    /// - Text-only content → no vision required
    /// - Mixed (text + image) → requires vision
    pub fn detect_vision_required(request: &Value) -> bool {
        // Check messages array for image content
        if let Some(messages) = request.get("messages").and_then(|m| m.as_array()) {
            return Self::contains_image_in_messages(messages);
        }

        // Check content field directly (single message format)
        if let Some(content) = request.get("content") {
            return Self::content_contains_image(content);
        }

        false
    }

    /// Check if messages array contains any image content
    fn contains_image_in_messages(messages: &[Value]) -> bool {
        messages.iter().any(|msg| {
            // Check content field
            if let Some(content) = msg.get("content") {
                Self::content_contains_image(content)
            } else {
                false
            }
        })
    }

    /// Check if content value (string, array, or object) contains images
    fn content_contains_image(content: &Value) -> bool {
        // String content - check for image URLs or data URIs
        if let Some(text) = content.as_str() {
            return Self::text_contains_image_url(text);
        }

        // Array content - check each element
        if let Some(arr) = content.as_array() {
            return arr.iter().any(Self::content_item_contains_image);
        }

        // Object content - check type field
        if let Some(obj) = content.as_object() {
            if let Some(type_str) = obj.get("type").and_then(|t| t.as_str()) {
                return type_str == "image" || type_str == "image_url";
            }
        }

        false
    }

    /// Check if a content array item contains an image
    fn content_item_contains_image(item: &Value) -> bool {
        // Check type field for image types
        if let Some(type_str) = item.get("type").and_then(|t| t.as_str()) {
            if type_str == "image" || type_str == "image_url" {
                return true;
            }
        }

        // Check nested content field
        if let Some(nested) = item.get("content") {
            if Self::content_contains_image(nested) {
                return true;
            }
        }

        // Check image_url field (OpenAI format)
        if item.get("image_url").is_some() {
            return true;
        }

        // Check source field (Anthropic format)
        if let Some(source) = item.get("source") {
            if let Some(type_str) = source.get("type").and_then(|t| t.as_str()) {
                if type_str == "base64" {
                    // Base64 encoded image
                    return true;
                }
            }
        }

        false
    }

    /// Check if text contains image URL patterns
    fn text_contains_image_url(text: &str) -> bool {
        let text_lower = text.to_lowercase();

        // Common image file extensions
        let image_extensions = [
            ".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".svg",
            ".tiff", ".ico", ".heic", ".avif",
        ];

        // Data URI pattern for images
        if text_lower.contains("data:image/") {
            return true;
        }

        // Check for image URLs
        for ext in &image_extensions {
            if text_lower.contains(ext) {
                return true;
            }
        }

        false
    }
}

/// Detector for tool/function calling requirements
pub struct ToolDetector;

impl ToolDetector {
    /// Detect if the request requires tool/function calling capability
    ///
    /// Checks for tool definitions in various formats:
    /// - OpenAI: `tools` array with function definitions
    /// - Anthropic: `tools` array
    /// - Legacy: `functions` array
    ///
    /// Empty arrays are treated as "no requirement" (optional feature).
    ///
    /// # Scenarios
    /// - Non-empty tool definitions → requires tools
    /// - No tool field → no requirement
    /// - Empty array → no requirement
    pub fn detect_tools_required(request: &Value) -> bool {
        // Check for tools array (modern format)
        if let Some(tools) = request.get("tools").and_then(|t| t.as_array()) {
            // Non-empty array requires tools
            return !tools.is_empty();
        }

        // Check for functions array (legacy format)
        if let Some(functions) = request.get("functions").and_then(|f| f.as_array()) {
            // Non-empty array requires tools
            return !functions.is_empty();
        }

        // Check for tool_choice field (indicates tools are being used)
        if request.get("tool_choice").is_some() {
            if let Some(choice) = request.get("tool_choice") {
                // Check if it's not "none"
                if let Some(choice_str) = choice.as_str() {
                    return choice_str != "none";
                }
                // Non-null object/bool means tools are active
                return !choice.is_null();
            }
        }

        // Check individual messages for tool_calls or tool_use
        if let Some(messages) = request.get("messages").and_then(|m| m.as_array()) {
            for msg in messages {
                // OpenAI format: tool_calls in message
                if msg.get("tool_calls").and_then(|t| t.as_array()).map(|a| !a.is_empty()).unwrap_or(false) {
                    return true;
                }

                // Anthropic format: tool_use content block
                if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
                    if content.iter().any(|block| {
                        block.get("type").and_then(|t| t.as_str()) == Some("tool_use")
                    }) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Check if tools are explicitly disabled (tool_choice = "none" or empty array)
    pub fn is_tools_disabled(request: &Value) -> bool {
        if let Some(choice) = request.get("tool_choice") {
            if let Some(choice_str) = choice.as_str() {
                return choice_str == "none";
            }
        }

        // Empty tools array means no tools
        if let Some(tools) = request.get("tools").and_then(|t| t.as_array()) {
            return tools.is_empty();
        }

        false
    }
}

/// Extractor for streaming preference from requests
pub struct StreamingExtractor;

impl StreamingExtractor {
    /// Extract streaming preference from request parameters
    ///
    /// Checks the `stream` parameter for explicit streaming preference.
    /// Returns false if the parameter is missing (default behavior).
    ///
    /// # Scenarios
    /// - `stream: true` → streaming required
    /// - `stream: false` → streaming not required
    /// - Missing stream parameter → default (false)
    pub fn extract_streaming_preference(request: &Value) -> bool {
        // Direct stream parameter
        if let Some(stream) = request.get("stream") {
            if let Some(stream_bool) = stream.as_bool() {
                return stream_bool;
            }
        }

        // Check in individual messages (some APIs set this per message)
        if let Some(messages) = request.get("messages").and_then(|m| m.as_array()) {
            for msg in messages {
                if let Some(stream) = msg.get("stream") {
                    if let Some(stream_bool) = stream.as_bool() {
                        return stream_bool;
                    }
                }
            }
        }

        // Default: no streaming required
        false
    }

    /// Check if streaming is explicitly disabled
    pub fn is_streaming_disabled(request: &Value) -> bool {
        if let Some(stream) = request.get("stream") {
            if let Some(stream_bool) = stream.as_bool() {
                return !stream_bool;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // ContentTypeDetector Tests
    // ========================================

    #[test]
    fn test_vision_detection_with_image_url() {
        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "image_url",
                    "image_url": {
                        "url": "https://example.com/image.png"
                    }
                }]
            }]
        });

        assert!(ContentTypeDetector::detect_vision_required(&request));
    }

    #[test]
    fn test_vision_detection_text_only() {
        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": "Hello, world!"
            }]
        });

        assert!(!ContentTypeDetector::detect_vision_required(&request));
    }

    #[test]
    fn test_vision_detection_mixed_content() {
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

    #[test]
    fn test_vision_detection_anthropic_format() {
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

        assert!(ContentTypeDetector::detect_vision_required(&request));
    }

    #[test]
    fn test_vision_detection_data_uri() {
        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": "Check out this image: data:image/png;base64,iVBORw0KG..."
            }]
        });

        assert!(ContentTypeDetector::detect_vision_required(&request));
    }

    #[test]
    fn test_vision_detection_no_messages() {
        let request = serde_json::json!({
            "model": "gpt-4",
            "prompt": "Hello"
        });

        assert!(!ContentTypeDetector::detect_vision_required(&request));
    }

    #[test]
    fn test_vision_detection_image_extension_in_text() {
        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": "Please analyze https://example.com/photo.jpg"
            }]
        });

        assert!(ContentTypeDetector::detect_vision_required(&request));
    }

    // ========================================
    // ToolDetector Tests
    // ========================================

    #[test]
    fn test_tool_detection_with_tools() {
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

    #[test]
    fn test_tool_detection_no_tools() {
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });

        assert!(!ToolDetector::detect_tools_required(&request));
    }

    #[test]
    fn test_tool_detection_empty_tools_array() {
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "tools": []
        });

        assert!(!ToolDetector::detect_tools_required(&request));
    }

    #[test]
    fn test_tool_detection_legacy_functions() {
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "functions": [{
                "name": "calculate",
                "parameters": {}
            }]
        });

        assert!(ToolDetector::detect_tools_required(&request));
    }

    #[test]
    fn test_tool_detection_tool_choice_auto() {
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "tool_choice": "auto"
        });

        assert!(ToolDetector::detect_tools_required(&request));
    }

    #[test]
    fn test_tool_detection_tool_choice_none() {
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "tool_choice": "none"
        });

        assert!(!ToolDetector::detect_tools_required(&request));
    }

    #[test]
    fn test_tool_detection_with_tool_calls() {
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

        assert!(ToolDetector::detect_tools_required(&request));
    }

    #[test]
    fn test_tool_detection_anthropic_tool_use() {
        let request = serde_json::json!({
            "messages": [{
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": "toolu_123",
                    "name": "get_weather",
                    "input": {}
                }]
            }]
        });

        assert!(ToolDetector::detect_tools_required(&request));
    }

    #[test]
    fn test_tool_detector_is_tools_disabled() {
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "tool_choice": "none"
        });

        assert!(ToolDetector::is_tools_disabled(&request));
    }

    #[test]
    fn test_tool_detector_empty_tools_disabled() {
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "tools": []
        });

        assert!(ToolDetector::is_tools_disabled(&request));
    }

    // ========================================
    // StreamingExtractor Tests
    // ========================================

    #[test]
    fn test_streaming_extraction_true() {
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": true
        });

        assert!(StreamingExtractor::extract_streaming_preference(&request));
    }

    #[test]
    fn test_streaming_extraction_false() {
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": false
        });

        assert!(!StreamingExtractor::extract_streaming_preference(&request));
    }

    #[test]
    fn test_streaming_extraction_default() {
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });

        assert!(!StreamingExtractor::extract_streaming_preference(&request));
    }

    #[test]
    fn test_streaming_extraction_missing_field() {
        let request = serde_json::json!({
            "model": "gpt-4",
            "prompt": "Hello"
        });

        assert!(!StreamingExtractor::extract_streaming_preference(&request));
    }

    #[test]
    fn test_streaming_is_disabled() {
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": false
        });

        assert!(StreamingExtractor::is_streaming_disabled(&request));
    }

    #[test]
    fn test_streaming_is_not_disabled() {
        let request = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": true
        });

        assert!(!StreamingExtractor::is_streaming_disabled(&request));
    }

    // ========================================
    // Edge Cases and Combined Tests
    // ========================================

    #[test]
    fn test_vision_detection_empty_messages() {
        let request = serde_json::json!({
            "messages": []
        });

        assert!(!ContentTypeDetector::detect_vision_required(&request));
    }

    #[test]
    fn test_vision_detection_null_content() {
        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": null
            }]
        });

        assert!(!ContentTypeDetector::detect_vision_required(&request));
    }

    #[test]
    fn test_tool_detection_multiple_messages_with_tools() {
        let request = serde_json::json!({
            "messages": [
                {"role": "user", "content": "What's the weather?"},
                {"role": "assistant", "content": "I'll check."},
                {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call_123",
                        "type": "function",
                        "function": {"name": "get_weather", "arguments": "{}"}
                    }]
                }
            ]
        });

        assert!(ToolDetector::detect_tools_required(&request));
    }

    #[test]
    fn test_all_detectors_complex_request() {
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
    }

    #[test]
    fn test_all_detectors_minimal_request() {
        let request = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": "Hello, world!"
            }]
        });

        assert!(!ContentTypeDetector::detect_vision_required(&request));
        assert!(!ToolDetector::detect_tools_required(&request));
        assert!(!StreamingExtractor::extract_streaming_preference(&request));
    }
}
