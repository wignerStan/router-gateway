//! `OpenAI` API adapter
//!
//! Transforms between gateway format and `OpenAI`'s Chat Completions API.

use super::types::{ProviderAdapter, ProviderRequest, ProviderResponse};
use anyhow::Result;
use serde_json::{Value, json};

/// `OpenAI` API adapter
pub struct OpenAIAdapter {
    default_base_url: String,
}

impl Default for OpenAIAdapter {
    fn default() -> Self {
        Self {
            default_base_url: "https://api.openai.com/v1".to_string(),
        }
    }
}

impl OpenAIAdapter {
    /// Create a new `OpenAI` adapter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an `OpenAI` adapter with a custom base URL.
    #[must_use]
    pub const fn with_base_url(base_url: String) -> Self {
        Self {
            default_base_url: base_url,
        }
    }
}

impl ProviderAdapter for OpenAIAdapter {
    fn provider_name(&self) -> &'static str {
        "openai"
    }

    fn transform_request(&self, request: &ProviderRequest) -> Value {
        use super::types::{MessageContent, ToolChoice};

        // Transform messages to OpenAI format
        let mut messages: Vec<Value> = Vec::new();

        for msg in &request.messages {
            let role = &msg.role;
            let content = match &msg.content {
                MessageContent::Text(text) => json!(text),
                MessageContent::Parts(parts) => {
                    let openai_parts: Vec<Value> = parts
                        .iter()
                        .map(|p| {
                            if p.part_type == "text" {
                                json!({
                                    "type": "text",
                                    "text": p.text.as_ref().unwrap_or(&String::new())
                                })
                            } else if p.part_type == "image_url" {
                                // Clippy suggests map_or_else but the branching logic is clearer as if-let
                                #[allow(clippy::option_if_let_else)]
                                if let Some(ref img_data) = p.image_data {
                                    json!({
                                        "type": "image_url",
                                        "image_url": {
                                            "url": format!("data:{};base64,{}", img_data.mime_type, img_data.data),
                                            "detail": p.image_url.as_ref().and_then(|u| u.detail.as_deref())
                                        }
                                    })
                                } else {
                                    json!({
                                        "type": "image_url",
                                        "image_url": {
                                            "url": p.image_url.as_ref().map_or(&String::new(), |u| &u.url),
                                            "detail": p.image_url.as_ref().and_then(|u| u.detail.as_deref())
                                        }
                                    })
                                }
                            } else {
                                json!({ "type": &p.part_type })
                            }
                        })
                        .collect();
                    json!(openai_parts)
                },
            };

            messages.push(json!({
                "role": role,
                "content": content
            }));
        }

        // Build request
        let mut openai_request = json!({
            "model": request.model,
            "messages": messages,
        });

        // Add system prompt if provided via the system field
        if let Some(system) = &request.system {
            if let Some(msgs) = openai_request["messages"].as_array_mut() {
                msgs.insert(
                    0,
                    json!({
                        "role": "system",
                        "content": system
                    }),
                );
            }
        }

        // Add optional parameters
        if let Some(max_tokens) = request.max_tokens {
            openai_request["max_tokens"] = json!(max_tokens);
        }
        if let Some(temp) = request.temperature {
            openai_request["temperature"] = json!(temp);
        }
        if let Some(top_p) = request.top_p {
            openai_request["top_p"] = json!(top_p);
        }
        if let Some(stop) = &request.stop {
            openai_request["stop"] = json!(stop);
        }
        if request.stream {
            openai_request["stream"] = json!(true);
        }

        // Transform tools
        if let Some(tools) = &request.tools {
            let openai_tools: Vec<Value> = tools
                .iter()
                .map(|t| {
                    json!({
                        "type": &t.tool_type,
                        "function": {
                            "name": t.function.name,
                            "description": t.function.description,
                            "parameters": t.function.parameters.clone().unwrap_or_else(|| json!({}))
                        }
                    })
                })
                .collect();
            openai_request["tools"] = json!(openai_tools);

            // Transform tool_choice
            if let Some(choice) = &request.tool_choice {
                let openai_choice = match choice {
                    ToolChoice::Auto => json!("auto"),
                    ToolChoice::None => json!("none"),
                    ToolChoice::Required => json!("required"),
                    ToolChoice::Function { name } => {
                        json!({"type": "function", "function": {"name": name}})
                    },
                };
                openai_request["tool_choice"] = openai_choice;
            }
        }

        openai_request
    }

    fn transform_response(&self, response: Value) -> Result<ProviderResponse> {
        use super::types::{FunctionCall, TokenUsage, ToolCall};

        let id = response["id"].as_str().unwrap_or("unknown").to_string();
        let model = response["model"].as_str().unwrap_or("unknown").to_string();

        // Extract content from OpenAI response
        let choices = response["choices"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("No choices in OpenAI response"))?;

        let first_choice = choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("Empty choices array"))?;

        let content = first_choice["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let finish_reason = first_choice["finish_reason"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        // Extract token usage
        let usage = &response["usage"];
        let token_usage = TokenUsage {
            prompt_tokens: usage["prompt_tokens"]
                .as_u64()
                .unwrap_or(0)
                .min(u64::from(u32::MAX)) as u32,
            completion_tokens: usage["completion_tokens"]
                .as_u64()
                .unwrap_or(0)
                .min(u64::from(u32::MAX)) as u32,
            total_tokens: usage["total_tokens"]
                .as_u64()
                .unwrap_or(0)
                .min(u64::from(u32::MAX)) as u32,
        };

        // Extract tool calls if present
        let tool_calls: Option<Vec<ToolCall>> = first_choice["message"]["tool_calls"]
            .as_array()
            .map(|calls| {
                calls
                    .iter()
                    .filter_map(|c| {
                        Some(ToolCall {
                            id: c["id"].as_str()?.to_string(),
                            call_type: c["type"].as_str().unwrap_or("function").to_string(),
                            function: FunctionCall {
                                name: c["function"]["name"].as_str()?.to_string(),
                                arguments: c["function"]["arguments"].as_str()?.to_string(),
                            },
                        })
                    })
                    .collect()
            });

        Ok(ProviderResponse {
            id,
            model,
            content,
            finish_reason,
            usage: token_usage,
            tool_calls,
        })
    }

    fn get_endpoint(&self, base_url: Option<&str>, _model_id: &str) -> String {
        // Handle empty base URL by falling back to default
        let base = match base_url {
            Some(url) if !url.is_empty() => url,
            _ => &self.default_base_url,
        };
        // Remove trailing slash to prevent double-slash issues
        let base = base.trim_end_matches('/');
        format!("{base}/chat/completions")
    }

    fn build_headers(&self, api_key: &str) -> Vec<(String, String)> {
        vec![
            ("Authorization".to_string(), format!("Bearer {api_key}")),
            ("Content-Type".to_string(), "application/json".to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::{
        ContentPart, FunctionDef, ImageUrl, Message, MessageContent, Tool, ToolChoice,
    };
    use super::*;
    use serde_json::json;

    // ============================================================
    // Basic Functionality Tests
    // ============================================================

    #[test]
    fn test_transform_simple_request() {
        let adapter = OpenAIAdapter::new();
        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text("Hello".to_string()),
                name: None,
            }],
            model: "gpt-4".to_string(),
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
        assert_eq!(transformed["model"], "gpt-4");
        assert_eq!(transformed["max_tokens"], 1024);
        // Compare temperature with approximate equality due to f32 precision
        let temp = transformed["temperature"].as_f64().unwrap();
        assert!(
            (temp - 0.7).abs() < 0.001,
            "Expected temperature ~0.7, got {temp}"
        );
    }

    // ============================================================
    // Provider Adapter Trait Tests
    // ============================================================

    #[test]
    fn test_provider_name() {
        let adapter = OpenAIAdapter::new();
        assert_eq!(adapter.provider_name(), "openai");
    }

    #[test]
    fn test_default_implementation() {
        let adapter = OpenAIAdapter::default();
        assert_eq!(adapter.default_base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn test_with_base_url_constructor() {
        let adapter = OpenAIAdapter::with_base_url("https://proxy.openai.com/v2".to_string());
        assert_eq!(adapter.default_base_url, "https://proxy.openai.com/v2");
    }

    // ============================================================
    // get_endpoint Tests
    // ============================================================

    #[test]
    fn test_get_endpoint_default_base_url() {
        let adapter = OpenAIAdapter::new();
        let endpoint = adapter.get_endpoint(None, "gpt-4");
        assert_eq!(endpoint, "https://api.openai.com/v1/chat/completions");
    }

    #[test]
    fn test_get_endpoint_custom_base_url() {
        let adapter = OpenAIAdapter::new();
        let endpoint = adapter.get_endpoint(Some("https://custom.api.com"), "gpt-4");
        assert_eq!(endpoint, "https://custom.api.com/chat/completions");
    }

    #[test]
    fn test_get_endpoint_custom_base_url_with_trailing_slash() {
        let adapter = OpenAIAdapter::new();
        let endpoint = adapter.get_endpoint(Some("https://custom.api.com/"), "gpt-4");
        // Trailing slash should be normalized to prevent double-slash issues
        assert_eq!(endpoint, "https://custom.api.com/chat/completions");
    }

    #[test]
    fn test_get_endpoint_with_base_url_from_constructor() {
        let adapter = OpenAIAdapter::with_base_url("https://proxy.example.com/v2".to_string());
        let endpoint = adapter.get_endpoint(None, "gpt-4");
        assert_eq!(endpoint, "https://proxy.example.com/v2/chat/completions");
    }

    #[test]
    fn test_get_endpoint_empty_base_url_uses_default() {
        let adapter = OpenAIAdapter::new();
        // Empty string should fall back to default base URL
        let endpoint = adapter.get_endpoint(Some(""), "gpt-4");
        assert_eq!(endpoint, "https://api.openai.com/v1/chat/completions");
    }

    #[test]
    fn test_get_endpoint_localhost() {
        let adapter = OpenAIAdapter::new();
        let endpoint = adapter.get_endpoint(Some("http://localhost:8080"), "gpt-4");
        assert_eq!(endpoint, "http://localhost:8080/chat/completions");
    }

    #[test]
    fn test_get_endpoint_ipv4_address() {
        let adapter = OpenAIAdapter::new();
        let endpoint = adapter.get_endpoint(Some("http://192.168.1.1:3000"), "gpt-4");
        assert_eq!(endpoint, "http://192.168.1.1:3000/chat/completions");
    }

    // ============================================================
    // build_headers Tests
    // ============================================================

    #[test]
    fn test_build_headers_format() {
        let adapter = OpenAIAdapter::new();
        let headers = adapter.build_headers("test-api-key");
        assert_eq!(headers.len(), 2);
        assert!(headers.contains(&(
            "Authorization".to_string(),
            "Bearer test-api-key".to_string()
        )));
        assert!(headers.contains(&("Content-Type".to_string(), "application/json".to_string())));
    }

    #[test]
    fn test_build_headers_empty_api_key() {
        let adapter = OpenAIAdapter::new();
        let headers = adapter.build_headers("");
        assert!(headers.contains(&("Authorization".to_string(), "Bearer ".to_string())));
    }

    #[test]
    fn test_build_headers_special_characters_in_key() {
        let adapter = OpenAIAdapter::new();
        let headers = adapter.build_headers("key-with-special!@#$%");
        assert!(headers.contains(&(
            "Authorization".to_string(),
            "Bearer key-with-special!@#$%".to_string()
        )));
    }

    // ============================================================
    // transform_request Edge Case Tests
    // ============================================================

    #[test]
    fn test_transform_request_empty_messages() {
        let adapter = OpenAIAdapter::new();
        let request = ProviderRequest {
            messages: vec![],
            model: "gpt-4".to_string(),
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
        let messages = transformed["messages"].as_array().unwrap();
        assert!(messages.is_empty());
    }

    #[test]
    fn test_transform_request_with_parts_content() {
        let adapter = OpenAIAdapter::new();
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
                            url: "https://example.com/image.png".to_string(),
                            detail: Some("high".to_string()),
                        }),
                        image_data: None,
                    },
                ]),
                name: None,
            }],
            model: "gpt-4-vision".to_string(),
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
        let messages = transformed["messages"].as_array().unwrap();
        let content = &messages[0]["content"];
        let parts = content.as_array().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[1]["type"], "image_url");
    }

    #[test]
    fn test_transform_request_with_tools() {
        let adapter = OpenAIAdapter::new();
        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text("What's the weather?".to_string()),
                name: None,
            }],
            model: "gpt-4".to_string(),
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
                    parameters: Some(
                        json!({"type": "object", "properties": {"location": {"type": "string"}}}),
                    ),
                },
            }]),
            tool_choice: Some(ToolChoice::Auto),
        };

        let transformed = adapter.transform_request(&request);
        assert!(transformed.get("tools").is_some());
        assert_eq!(transformed["tool_choice"], "auto");
    }

    #[test]
    fn test_transform_request_with_tool_choice_none() {
        let adapter = OpenAIAdapter::new();
        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text("Hello".to_string()),
                name: None,
            }],
            model: "gpt-4".to_string(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            system: None,
            tools: Some(vec![Tool {
                tool_type: "function".to_string(),
                function: FunctionDef {
                    name: "test".to_string(),
                    description: Some("Test".to_string()),
                    parameters: None,
                },
            }]),
            tool_choice: Some(ToolChoice::None),
        };

        let transformed = adapter.transform_request(&request);
        assert_eq!(transformed["tool_choice"], "none");
    }

    #[test]
    fn test_transform_request_with_tool_choice_required() {
        let adapter = OpenAIAdapter::new();
        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text("Hello".to_string()),
                name: None,
            }],
            model: "gpt-4".to_string(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            system: None,
            tools: Some(vec![Tool {
                tool_type: "function".to_string(),
                function: FunctionDef {
                    name: "test".to_string(),
                    description: Some("Test".to_string()),
                    parameters: None,
                },
            }]),
            tool_choice: Some(ToolChoice::Required),
        };

        let transformed = adapter.transform_request(&request);
        assert_eq!(transformed["tool_choice"], "required");
    }

    #[test]
    fn test_transform_request_with_tool_choice_function() {
        let adapter = OpenAIAdapter::new();
        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text("Hello".to_string()),
                name: None,
            }],
            model: "gpt-4".to_string(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            system: None,
            tools: Some(vec![Tool {
                tool_type: "function".to_string(),
                function: FunctionDef {
                    name: "test".to_string(),
                    description: Some("Test".to_string()),
                    parameters: None,
                },
            }]),
            tool_choice: Some(ToolChoice::Function {
                name: "test".to_string(),
            }),
        };

        let transformed = adapter.transform_request(&request);
        let choice = &transformed["tool_choice"];
        assert_eq!(choice["type"], "function");
        assert_eq!(choice["function"]["name"], "test");
    }

    #[test]
    fn test_transform_request_with_stream() {
        let adapter = OpenAIAdapter::new();
        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text("Hello".to_string()),
                name: None,
            }],
            model: "gpt-4".to_string(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
            stream: true,
            system: None,
            tools: None,
            tool_choice: None,
        };

        let transformed = adapter.transform_request(&request);
        assert_eq!(transformed["stream"], true);
    }

    #[test]
    fn test_transform_request_with_all_optional_fields() {
        let adapter = OpenAIAdapter::new();
        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text("Hello".to_string()),
                name: None,
            }],
            model: "gpt-4".to_string(),
            max_tokens: Some(2048),
            temperature: Some(0.5),
            top_p: Some(0.9),
            stop: Some(vec!["STOP".to_string(), "END".to_string()]),
            stream: true,
            system: Some("Be helpful".to_string()),
            tools: None,
            tool_choice: None,
        };

        let transformed = adapter.transform_request(&request);
        assert_eq!(transformed["max_tokens"], 2048);
        assert_eq!(transformed["stream"], true);
        let stop = transformed["stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2);
    }

    // ============================================================
    // transform_response Edge Case Tests
    // ============================================================

    #[test]
    fn test_transform_response() {
        let adapter = OpenAIAdapter::new();
        let response = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{
                "message": {"content": "Hello there!"},
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.id, "chatcmpl-123");
        assert_eq!(result.content, "Hello there!");
        assert_eq!(result.usage.total_tokens, 15);
    }

    #[test]
    fn test_transform_response_missing_choices() {
        let adapter = OpenAIAdapter::new();
        let response = json!({});
        let result = adapter.transform_response(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_response_empty_choices() {
        let adapter = OpenAIAdapter::new();
        let response = json!({"choices": []});
        let result = adapter.transform_response(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_response_missing_id() {
        let adapter = OpenAIAdapter::new();
        let response = json!({
            "choices": [{
                "message": {"content": "Hello"},
                "finish_reason": "stop"
            }],
            "usage": {}
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.id, "unknown");
    }

    #[test]
    fn test_transform_response_missing_model() {
        let adapter = OpenAIAdapter::new();
        let response = json!({
            "id": "chatcmpl-123",
            "choices": [{
                "message": {"content": "Hello"},
                "finish_reason": "stop"
            }],
            "usage": {}
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.model, "unknown");
    }

    #[test]
    fn test_transform_response_missing_content() {
        let adapter = OpenAIAdapter::new();
        let response = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{
                "message": {},
                "finish_reason": "stop"
            }],
            "usage": {}
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.content, "");
    }

    #[test]
    fn test_transform_response_missing_finish_reason() {
        let adapter = OpenAIAdapter::new();
        let response = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{
                "message": {"content": "Hello"}
            }],
            "usage": {}
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.finish_reason, "unknown");
    }

    #[test]
    fn test_transform_response_missing_usage() {
        let adapter = OpenAIAdapter::new();
        let response = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{
                "message": {"content": "Hello"},
                "finish_reason": "stop"
            }]
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.usage.prompt_tokens, 0);
        assert_eq!(result.usage.completion_tokens, 0);
        assert_eq!(result.usage.total_tokens, 0);
    }

    #[test]
    fn test_transform_response_with_tool_calls() {
        let adapter = OpenAIAdapter::new();
        let response = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{
                "message": {
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc123",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\": \"NYC\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {}
        });

        let result = adapter.transform_response(response).unwrap();
        assert!(result.tool_calls.is_some());
        let tool_calls = result.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_abc123");
        assert_eq!(tool_calls[0].function.name, "get_weather");
        assert_eq!(tool_calls[0].function.arguments, "{\"location\": \"NYC\"}");
    }

    #[test]
    fn test_transform_response_multiple_tool_calls() {
        let adapter = OpenAIAdapter::new();
        let response = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{
                "message": {
                    "content": null,
                    "tool_calls": [
                        {
                            "id": "call_1",
                            "type": "function",
                            "function": {"name": "func1", "arguments": "{}"}
                        },
                        {
                            "id": "call_2",
                            "type": "function",
                            "function": {"name": "func2", "arguments": "{}"}
                        }
                    ]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {}
        });

        let result = adapter.transform_response(response).unwrap();
        let tool_calls = result.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 2);
    }

    #[test]
    fn test_transform_response_partial_usage() {
        let adapter = OpenAIAdapter::new();
        let response = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{
                "message": {"content": "Hello"},
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10
            }
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.usage.prompt_tokens, 10);
        assert_eq!(result.usage.completion_tokens, 0);
        assert_eq!(result.usage.total_tokens, 0);
    }

    mod proptests {
        use crate::providers::types::{Message, MessageContent, ProviderAdapter};
        use proptest::prelude::*;
        use serde_json::Value;

        use super::OpenAIAdapter;

        /// Recursive strategy for arbitrary JSON values (depth-limited to avoid stack overflow).
        fn arb_json_value() -> impl Strategy<Value = Value> {
            let leaf = prop_oneof![
                Just(Value::Null),
                proptest::arbitrary::any::<bool>().prop_map(Value::Bool),
                proptest::arbitrary::any::<f64>().prop_map(Value::from),
                proptest::arbitrary::any::<String>().prop_map(Value::String),
            ];
            leaf.prop_recursive(3, 16, 8, |inner| {
                prop_oneof![
                    prop::collection::vec(inner.clone(), 0..8).prop_map(Value::Array),
                    prop::collection::vec((proptest::arbitrary::any::<String>(), inner), 0..8)
                        .prop_map(|pairs| Value::Object(pairs.into_iter().collect())),
                ]
            })
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(64))]

            #[test]
            fn proptests_transform_response_never_panics(
                response in arb_json_value()
            ) {
                let adapter = OpenAIAdapter::new();
                // Must not panic on arbitrary JSON — may return Ok or Err
                let _ = adapter.transform_response(response);
            }

            #[test]
            fn transform_request_any_model(model in "\\PC*") {
                let adapter = OpenAIAdapter::new();
                let request = crate::providers::types::ProviderRequest {
                    messages: vec![Message {
                        role: "user".to_string(),
                        content: MessageContent::Text("Hello".to_string()),
                        name: None,
                    }],
                    model,
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
                // Model field must be present in output
                assert!(transformed.get("model").is_some());
                // Messages array must be present
                assert!(transformed.get("messages").is_some());
            }
        }
    }
}
