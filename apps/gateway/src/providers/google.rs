//! Google/Gemini API adapter
//!
//! Transforms between gateway format and Google's Gemini API.

use super::types::{ProviderAdapter, ProviderRequest, ProviderResponse};
use anyhow::Result;
use serde_json::{json, Value};

/// Google/Gemini API adapter
pub struct GoogleAdapter {
    default_base_url: String,
}

impl Default for GoogleAdapter {
    fn default() -> Self {
        Self {
            default_base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
        }
    }
}

impl GoogleAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: String) -> Self {
        Self {
            default_base_url: base_url,
        }
    }
}

impl ProviderAdapter for GoogleAdapter {
    fn provider_name(&self) -> &str {
        "google"
    }

    fn transform_request(&self, request: &ProviderRequest) -> Value {
        use super::types::MessageContent;

        // Transform messages to Gemini format
        let mut contents: Vec<Value> = Vec::new();
        let mut system_instruction: Option<Value> = None;

        for msg in &request.messages {
            // Extract system message
            if msg.role == "system" {
                match &msg.content {
                    MessageContent::Text(text) => {
                        system_instruction = Some(json!({
                            "parts": [{"text": text}]
                        }));
                    },
                    MessageContent::Parts(parts) => {
                        let texts: Vec<Value> = parts
                            .iter()
                            .filter_map(|p| p.text.as_ref().map(|t| json!({"text": t})))
                            .collect();
                        if !texts.is_empty() {
                            system_instruction = Some(json!({"parts": texts}));
                        }
                    },
                }
                continue;
            }

            // Map roles to Gemini format
            let gemini_role = match msg.role.as_str() {
                "assistant" => "model",
                "user" => "user",
                _ => msg.role.as_str(),
            };

            let parts = match &msg.content {
                MessageContent::Text(text) => {
                    vec![json!({"text": text})]
                },
                MessageContent::Parts(parts) => {
                    parts
                        .iter()
                        .map(|p| {
                            if p.part_type == "text" {
                                json!({"text": p.text})
                            } else if p.part_type == "image_url" {
                                // Gemini expects inline_data or fileData
                                json!({
                                    "inlineData": {
                                        "mimeType": "image/jpeg",
                                        "data": p.image_url.as_ref().map(|u| &u.url).unwrap_or(&String::new())
                                    }
                                })
                            } else {
                                json!({})
                            }
                        })
                        .collect()
                },
            };

            contents.push(json!({
                "role": gemini_role,
                "parts": parts
            }));
        }

        // Build generation config
        let mut generation_config = json!({});

        if let Some(max_tokens) = request.max_tokens {
            generation_config["maxOutputTokens"] = json!(max_tokens);
        }
        if let Some(temp) = request.temperature {
            generation_config["temperature"] = json!(temp);
        }
        if let Some(top_p) = request.top_p {
            generation_config["topP"] = json!(top_p);
        }
        if let Some(stop) = &request.stop {
            generation_config["stopSequences"] = json!(stop);
        }

        // Build request
        let mut gemini_request = json!({
            "contents": contents,
            "generationConfig": generation_config,
        });

        // Add system instruction if present
        if let Some(sys) = system_instruction.or(request
            .system
            .as_ref()
            .map(|s| json!({"parts": [{"text": s}]})))
        {
            gemini_request["systemInstruction"] = sys;
        }

        // Transform tools if present
        if let Some(tools) = &request.tools {
            let declarations: Vec<Value> = tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.function.name,
                        "description": t.function.description,
                        "parameters": t.function.parameters.clone().unwrap_or(json!({}))
                    })
                })
                .collect();

            gemini_request["tools"] = json!([{
                "functionDeclarations": declarations
            }]);
        }

        gemini_request
    }

    fn transform_response(&self, response: Value) -> Result<ProviderResponse> {
        use super::types::{FunctionCall, TokenUsage, ToolCall};

        // Extract content from Gemini response
        let candidates = response["candidates"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("No candidates in Gemini response"))?;

        let first_candidate = candidates
            .first()
            .ok_or_else(|| anyhow::anyhow!("Empty candidates array"))?;

        // Get text content
        let content: String = first_candidate["content"]["parts"]
            .as_array()
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|p| p["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        // Get finish reason
        let finish_reason = first_candidate["finishReason"]
            .as_str()
            .unwrap_or("UNKNOWN")
            .to_string();

        // Extract usage
        let usage = &response["usageMetadata"];
        let token_usage = TokenUsage {
            prompt_tokens: usage["promptTokenCount"].as_u64().unwrap_or(0) as u32,
            completion_tokens: usage["candidatesTokenCount"].as_u64().unwrap_or(0) as u32,
            total_tokens: usage["totalTokenCount"].as_u64().unwrap_or(0) as u32,
        };

        // Extract tool calls if present
        let tool_calls: Option<Vec<ToolCall>> =
            first_candidate["content"]["parts"].as_array().map(|parts| {
                parts
                    .iter()
                    .filter(|p| p.get("functionCall").is_some())
                    .filter_map(|p| {
                        let fc = p.get("functionCall")?;
                        Some(ToolCall {
                            id: format!("call_{}", uuid::Uuid::new_v4()),
                            call_type: "function".to_string(),
                            function: FunctionCall {
                                name: fc["name"].as_str()?.to_string(),
                                arguments: serde_json::to_string(&fc["args"]).unwrap_or_default(),
                            },
                        })
                    })
                    .collect()
            });

        Ok(ProviderResponse {
            id: format!("gemini-{}", uuid::Uuid::new_v4()),
            model: response
                .get("modelVersion")
                .and_then(|m| m.as_str())
                .unwrap_or("gemini")
                .to_string(),
            content,
            finish_reason,
            usage: token_usage,
            tool_calls,
        })
    }

    fn get_endpoint(&self, base_url: Option<&str>, model_id: &str) -> String {
        // Handle empty base URL by falling back to default
        let base = match base_url {
            Some(url) if !url.is_empty() => url,
            _ => &self.default_base_url,
        };
        // Remove trailing slash to prevent double-slash issues
        let base = base.trim_end_matches('/');
        // Sanitize model_id to prevent path traversal/SSRF attacks
        // Remove any path traversal characters
        let sanitized_model_id = model_id.replace(['/', '\\'], "");
        format!("{}/models/{}:generateContent", base, sanitized_model_id)
    }

    fn build_headers(&self, api_key: &str) -> Vec<(String, String)> {
        vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("x-goog-api-key".to_string(), api_key.to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::{ContentPart, FunctionDef, ImageUrl, Message, MessageContent, Tool};
    use super::*;

    // ============================================================
    // Basic Functionality Tests
    // ============================================================

    #[test]
    fn test_transform_simple_request() {
        let adapter = GoogleAdapter::new();
        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text("Hello".to_string()),
                name: None,
            }],
            model: "gemini-pro".to_string(),
            max_tokens: Some(1024),
            temperature: Some(0.7),
            top_p: None,
            stop: None,
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
        };

        let transformed = adapter.transform_request(&request);
        assert!(transformed.get("contents").is_some());
        assert!(transformed.get("generationConfig").is_some());
    }

    // ============================================================
    // Provider Adapter Trait Tests
    // ============================================================

    #[test]
    fn test_provider_name() {
        let adapter = GoogleAdapter::new();
        assert_eq!(adapter.provider_name(), "google");
    }

    #[test]
    fn test_default_implementation() {
        let adapter = GoogleAdapter::default();
        assert_eq!(
            adapter.default_base_url,
            "https://generativelanguage.googleapis.com/v1beta"
        );
    }

    #[test]
    fn test_with_base_url_constructor() {
        let adapter = GoogleAdapter::with_base_url("https://custom.google.api.com/v2".to_string());
        assert_eq!(adapter.default_base_url, "https://custom.google.api.com/v2");
    }

    // ============================================================
    // get_endpoint Tests
    // ============================================================

    #[test]
    fn test_get_endpoint_default_base_url() {
        let adapter = GoogleAdapter::new();
        let endpoint = adapter.get_endpoint(None, "gemini-pro");
        assert_eq!(
            endpoint,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent"
        );
    }

    #[test]
    fn test_get_endpoint_custom_base_url() {
        let adapter = GoogleAdapter::new();
        let endpoint = adapter.get_endpoint(Some("https://custom.api.com"), "gemini-pro");
        assert_eq!(
            endpoint,
            "https://custom.api.com/models/gemini-pro:generateContent"
        );
    }

    #[test]
    fn test_get_endpoint_custom_base_url_with_trailing_slash() {
        let adapter = GoogleAdapter::new();
        let endpoint = adapter.get_endpoint(Some("https://custom.api.com/"), "gemini-pro");
        // Trailing slash should be normalized to prevent double-slash issues
        assert_eq!(
            endpoint,
            "https://custom.api.com/models/gemini-pro:generateContent"
        );
    }

    #[test]
    fn test_get_endpoint_with_base_url_from_constructor() {
        let adapter = GoogleAdapter::with_base_url("https://proxy.example.com/v2".to_string());
        let endpoint = adapter.get_endpoint(None, "gemini-pro");
        assert_eq!(
            endpoint,
            "https://proxy.example.com/v2/models/gemini-pro:generateContent"
        );
    }

    #[test]
    fn test_get_endpoint_empty_base_url_uses_default() {
        let adapter = GoogleAdapter::new();
        // Empty string should fall back to default base URL
        let endpoint = adapter.get_endpoint(Some(""), "gemini-pro");
        assert_eq!(
            endpoint,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent"
        );
    }

    #[test]
    fn test_get_endpoint_model_id_sanitized() {
        let adapter = GoogleAdapter::new();
        // Path traversal characters (/ and \) should be removed, leaving dots
        // "../../../etc/passwd" has 6 dots total: ..(2) + ..(2) + ...(3) - wait no, let's verify:
        // After removing all '/' chars: ......etcpasswd (6 dots)
        let endpoint = adapter.get_endpoint(None, "../../../etc/passwd");
        assert_eq!(
            endpoint,
            "https://generativelanguage.googleapis.com/v1beta/models/......etcpasswd:generateContent"
        );
    }

    #[test]
    fn test_get_endpoint_localhost() {
        let adapter = GoogleAdapter::new();
        let endpoint = adapter.get_endpoint(Some("http://localhost:8080"), "gemini-pro");
        assert_eq!(
            endpoint,
            "http://localhost:8080/models/gemini-pro:generateContent"
        );
    }

    // ============================================================
    // build_headers Tests
    // ============================================================

    #[test]
    fn test_build_headers_format() {
        let adapter = GoogleAdapter::new();
        let headers = adapter.build_headers("test-api-key");
        assert_eq!(headers.len(), 2);
        assert!(headers.contains(&("Content-Type".to_string(), "application/json".to_string())));
        assert!(headers.contains(&("x-goog-api-key".to_string(), "test-api-key".to_string())));
    }

    #[test]
    fn test_build_headers_empty_api_key() {
        let adapter = GoogleAdapter::new();
        let headers = adapter.build_headers("");
        assert!(headers.contains(&("x-goog-api-key".to_string(), "".to_string())));
    }

    // ============================================================
    // transform_request Edge Case Tests
    // ============================================================

    #[test]
    fn test_transform_request_with_system_message() {
        let adapter = GoogleAdapter::new();
        let request = ProviderRequest {
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: MessageContent::Text("You are helpful".to_string()),
                    name: None,
                },
                Message {
                    role: "user".to_string(),
                    content: MessageContent::Text("Hello".to_string()),
                    name: None,
                },
            ],
            model: "gemini-pro".to_string(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
        };

        let transformed = adapter.transform_request(&request);
        assert!(transformed.get("systemInstruction").is_some());
    }

    #[test]
    fn test_transform_request_with_system_parts() {
        let adapter = GoogleAdapter::new();
        let request = ProviderRequest {
            messages: vec![Message {
                role: "system".to_string(),
                content: MessageContent::Parts(vec![ContentPart {
                    part_type: "text".to_string(),
                    text: Some("System instruction".to_string()),
                    image_url: None,
                    image_data: None,
                }]),
                name: None,
            }],
            model: "gemini-pro".to_string(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
        };

        let transformed = adapter.transform_request(&request);
        assert!(transformed.get("systemInstruction").is_some());
    }

    #[test]
    fn test_transform_request_with_assistant_message() {
        let adapter = GoogleAdapter::new();
        let request = ProviderRequest {
            messages: vec![
                Message {
                    role: "user".to_string(),
                    content: MessageContent::Text("Hello".to_string()),
                    name: None,
                },
                Message {
                    role: "assistant".to_string(),
                    content: MessageContent::Text("Hi there!".to_string()),
                    name: None,
                },
            ],
            model: "gemini-pro".to_string(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
        };

        let transformed = adapter.transform_request(&request);
        let contents = transformed["contents"].as_array().unwrap();
        assert_eq!(contents[1]["role"], "model");
    }

    #[test]
    fn test_transform_request_with_tools() {
        let adapter = GoogleAdapter::new();
        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text("Hello".to_string()),
                name: None,
            }],
            model: "gemini-pro".to_string(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            system: None,
            tools: Some(vec![Tool {
                tool_type: "function".to_string(),
                function: FunctionDef {
                    name: "get_weather".to_string(),
                    description: Some("Get weather info".to_string()),
                    parameters: Some(json!({"type": "object"})),
                },
            }]),
            tool_choice: None,
        };

        let transformed = adapter.transform_request(&request);
        assert!(transformed.get("tools").is_some());
    }

    #[test]
    fn test_transform_request_with_image_parts() {
        let adapter = GoogleAdapter::new();
        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Parts(vec![
                    ContentPart {
                        part_type: "text".to_string(),
                        text: Some("What's in this image?".to_string()),
                        image_url: None,
                        image_data: None,
                    },
                    ContentPart {
                        part_type: "image_url".to_string(),
                        text: None,
                        image_url: Some(ImageUrl {
                            url: "base64encodeddata".to_string(),
                            detail: None,
                        }),
                        image_data: None,
                    },
                ]),
                name: None,
            }],
            model: "gemini-pro".to_string(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
        };

        let transformed = adapter.transform_request(&request);
        let contents = transformed["contents"].as_array().unwrap();
        let parts = contents[0]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn test_transform_request_with_all_config() {
        let adapter = GoogleAdapter::new();
        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text("Hello".to_string()),
                name: None,
            }],
            model: "gemini-pro".to_string(),
            max_tokens: Some(2048),
            temperature: Some(0.5),
            top_p: Some(0.9),
            stop: Some(vec!["STOP".to_string()]),
            stream: false,
            system: Some("Be helpful".to_string()),
            tools: None,
            tool_choice: None,
        };

        let transformed = adapter.transform_request(&request);
        let gen_config = &transformed["generationConfig"];
        assert_eq!(gen_config["maxOutputTokens"], 2048);
        assert_eq!(gen_config["temperature"], 0.5);
        // Use approximate comparison for top_p due to floating point precision
        let top_p = gen_config["topP"].as_f64().unwrap();
        assert!(
            (top_p - 0.9).abs() < 0.01,
            "Expected topP ~0.9, got {}",
            top_p
        );
        assert!(gen_config.get("stopSequences").is_some());
        assert!(transformed.get("systemInstruction").is_some());
    }

    // ============================================================
    // transform_response Edge Case Tests
    // ============================================================

    #[test]
    fn test_transform_response_success() {
        let adapter = GoogleAdapter::new();
        let response = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello there!"}]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 5,
                "totalTokenCount": 15
            },
            "modelVersion": "gemini-1.5-pro"
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.content, "Hello there!");
        assert_eq!(result.finish_reason, "STOP");
        assert_eq!(result.usage.prompt_tokens, 10);
        assert_eq!(result.usage.completion_tokens, 5);
        assert_eq!(result.usage.total_tokens, 15);
        assert_eq!(result.model, "gemini-1.5-pro");
    }

    #[test]
    fn test_transform_response_missing_candidates() {
        let adapter = GoogleAdapter::new();
        let response = json!({});
        let result = adapter.transform_response(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_response_empty_candidates() {
        let adapter = GoogleAdapter::new();
        let response = json!({"candidates": []});
        let result = adapter.transform_response(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_response_missing_usage() {
        let adapter = GoogleAdapter::new();
        let response = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello"}]
                },
                "finishReason": "STOP"
            }]
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.usage.prompt_tokens, 0);
        assert_eq!(result.usage.completion_tokens, 0);
        assert_eq!(result.usage.total_tokens, 0);
    }

    #[test]
    fn test_transform_response_with_tool_calls() {
        let adapter = GoogleAdapter::new();
        let response = json!({
            "candidates": [{
                "content": {
                    "parts": [
                        {"text": "Let me check that."},
                        {"functionCall": {"name": "get_weather", "args": {"location": "NYC"}}}
                    ]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 5,
                "totalTokenCount": 15
            }
        });

        let result = adapter.transform_response(response).unwrap();
        assert!(result.tool_calls.is_some());
        let tool_calls = result.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].function.name, "get_weather");
    }

    #[test]
    fn test_transform_response_multiple_text_parts() {
        let adapter = GoogleAdapter::new();
        let response = json!({
            "candidates": [{
                "content": {
                    "parts": [
                        {"text": "Hello "},
                        {"text": "there!"}
                    ]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {}
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.content, "Hello there!");
    }

    #[test]
    fn test_transform_response_no_content_parts() {
        let adapter = GoogleAdapter::new();
        let response = json!({
            "candidates": [{
                "content": {
                    "parts": []
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {}
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.content, "");
    }

    #[test]
    fn test_transform_response_missing_finish_reason() {
        let adapter = GoogleAdapter::new();
        let response = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello"}]
                }
            }],
            "usageMetadata": {}
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.finish_reason, "UNKNOWN");
    }
}
