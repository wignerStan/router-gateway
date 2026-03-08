use crate::classification::RequestFormat;
use serde_json::Value;

/// Detector for LLM request format/protocol
///
/// Analyzes the structure of a request to determine which provider's
/// format it uses (OpenAI, Anthropic, Gemini, or Generic).
pub struct FormatDetector;

impl FormatDetector {
    /// Detect the format of a request from its structure
    ///
    /// # Detection Rules
    ///
    /// - **OpenAI**: Has `messages` array with role/content fields
    /// - **Anthropic**: Has `messages` with user/assistant roles AND `system` field
    /// - **Gemini**: Has `contents` array with `parts` field
    /// - **Generic**: Unknown or ambiguous structure
    pub fn detect(request: &Value) -> RequestFormat {
        // Check for Gemini format (contents array with parts)
        if Self::is_gemini_format(request) {
            return RequestFormat::Gemini;
        }

        // Check for Anthropic format (messages + system field)
        if Self::is_anthropic_format(request) {
            return RequestFormat::Anthropic;
        }

        // Check for OpenAI format (messages array)
        if Self::is_openai_format(request) {
            return RequestFormat::OpenAI;
        }

        // Default to Generic for unknown formats
        RequestFormat::Generic
    }

    /// Check if request uses OpenAI format
    fn is_openai_format(request: &Value) -> bool {
        request.get("messages").and_then(|m| m.as_array()).is_some()
    }

    /// Check if request uses Anthropic format
    fn is_anthropic_format(request: &Value) -> bool {
        // Anthropic has messages AND a separate system field
        let has_messages = request.get("messages").and_then(|m| m.as_array()).is_some();

        let has_system = request.get("system").is_some();

        has_messages && has_system
    }

    /// Check if request uses Gemini format
    fn is_gemini_format(request: &Value) -> bool {
        // Gemini uses `contents` array instead of `messages`
        request.get("contents").and_then(|c| c.as_array()).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_detect_openai_format() {
        let request = json!({
            "messages": [
                {"role": "user", "content": "Hello"}
            ],
            "model": "gpt-4"
        });

        assert_eq!(FormatDetector::detect(&request), RequestFormat::OpenAI);
    }

    #[test]
    fn test_detect_anthropic_format() {
        let request = json!({
            "messages": [
                {"role": "user", "content": "Hello"}
            ],
            "system": "You are a helpful assistant",
            "model": "claude-3-opus"
        });

        assert_eq!(FormatDetector::detect(&request), RequestFormat::Anthropic);
    }

    #[test]
    fn test_detect_gemini_format() {
        let request = json!({
            "contents": [
                {
                    "parts": [{"text": "Hello"}]
                }
            ],
            "model": "gemini-pro"
        });

        assert_eq!(FormatDetector::detect(&request), RequestFormat::Gemini);
    }

    #[test]
    fn test_detect_generic_format() {
        let request = json!({
            "prompt": "Hello",
            "model": "unknown-model"
        });

        assert_eq!(FormatDetector::detect(&request), RequestFormat::Generic);
    }

    #[test]
    fn test_openai_without_system_is_still_openai() {
        let request = json!({
            "messages": [
                {"role": "user", "content": "Hello"}
            ],
            "model": "gpt-4"
        });

        assert_eq!(FormatDetector::detect(&request), RequestFormat::OpenAI);
    }

    #[test]
    fn test_anthropic_requires_system_field() {
        let request_without_system = json!({
            "messages": [
                {"role": "user", "content": "Hello"}
            ],
            "model": "claude-3-opus"
        });

        // Without system field, it's detected as OpenAI (has messages)
        assert_eq!(
            FormatDetector::detect(&request_without_system),
            RequestFormat::OpenAI
        );

        let request_with_system = json!({
            "messages": [
                {"role": "user", "content": "Hello"}
            ],
            "system": "You are helpful",
            "model": "claude-3-opus"
        });

        assert_eq!(
            FormatDetector::detect(&request_with_system),
            RequestFormat::Anthropic
        );
    }

    #[test]
    fn test_gemini_contents_takes_priority() {
        let request = json!({
            "contents": [
                {"parts": [{"text": "Hello"}]}
            ],
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        });

        // Gemini format takes priority (checked first)
        assert_eq!(FormatDetector::detect(&request), RequestFormat::Gemini);
    }
}
