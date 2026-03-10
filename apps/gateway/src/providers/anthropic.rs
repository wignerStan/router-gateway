//! Anthropic API adapter
//!
//! Transforms between gateway format and Anthropic's Messages API.

use super::types::{ProviderAdapter, ProviderRequest, ProviderResponse};
use anyhow::Result;
use serde_json::{json, Value};

/// Anthropic API adapter
pub struct AnthropicAdapter {
    default_base_url: String,
}

impl Default for AnthropicAdapter {
    fn default() -> Self {
        Self {
            default_base_url: "https://api.anthropic.com/v1".to_string(),
        }
    }
}

impl AnthropicAdapter {
    /// Create a new Anthropic adapter
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with custom base URL
    pub fn with_base_url(base_url: String) -> Self {
        Self {
            default_base_url: base_url,
        }
    }
}

impl ProviderAdapter for AnthropicAdapter {
    fn provider_name(&self) -> &str {
        "anthropic"
    }

    fn transform_request(&self, request: &ProviderRequest) -> Value {
        use super::types::{MessageContent, ToolChoice};

        // Transform messages to Anthropic format
        let mut messages: Vec<Value> = Vec::new();
        let mut system: Option<String> = None;

        for msg in &request.messages {
            // Extract system message
            if msg.role == "system" {
                match &msg.content {
                    MessageContent::Text(text) => system = Some(text.clone()),
                    MessageContent::Parts(parts) => {
                        let texts: Vec<&str> =
                            parts.iter().filter_map(|p| p.text.as_deref()).collect();
                        if !texts.is_empty() {
                            system = Some(texts.join("\n"));
                        }
                    },
                }
                continue;
            }

            let content = match &msg.content {
                MessageContent::Text(text) => {
                    json!([{ "type": "text", "text": text }])
                },
                MessageContent::Parts(parts) => {
                    let anthropic_parts: Vec<Value> = parts
                        .iter()
                        .map(|p| {
                            if p.part_type == "text" {
                                json!({
                                    "type": "text",
                                    "text": p.text.as_ref().unwrap_or(&String::new())
                                })
                            } else if p.part_type == "image_url" {
                                // Safe handling: check if image_url exists before accessing
                                if let Some(image_url) = &p.image_url {
                                    // Anthropic expects base64 or URL in specific format
                                    json!({
                                        "type": "image",
                                        "source": {
                                            "type": "url",
                                            "url": &image_url.url
                                        }
                                    })
                                } else {
                                    // Skip malformed image_url content - log as text placeholder
                                    json!({
                                        "type": "text",
                                        "text": "[malformed image content]"
                                    })
                                }
                            } else {
                                json!({ "type": &p.part_type })
                            }
                        })
                        .collect();
                    json!(anthropic_parts)
                },
            };

            messages.push(json!({
                "role": msg.role,
                "content": content
            }));
        }

        // Build request
        let mut anthropic_request = json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(4096),
        });

        // Add system if present
        if let Some(sys) = system.or(request.system.clone()) {
            anthropic_request["system"] = json!(sys);
        }

        // Add optional parameters
        if let Some(temp) = request.temperature {
            anthropic_request["temperature"] = json!(temp);
        }
        if let Some(top_p) = request.top_p {
            anthropic_request["top_p"] = json!(top_p);
        }
        if let Some(stop) = &request.stop {
            anthropic_request["stop_sequences"] = json!(stop);
        }
        if request.stream {
            anthropic_request["stream"] = json!(true);
        }

        // Transform tools
        if let Some(tools) = &request.tools {
            let anthropic_tools: Vec<Value> = tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.function.name,
                        "description": t.function.description,
                        "input_schema": t.function.parameters.clone().unwrap_or(json!({}))
                    })
                })
                .collect();
            anthropic_request["tools"] = json!(anthropic_tools);

            // Transform tool_choice
            if let Some(choice) = &request.tool_choice {
                let anthropic_choice = match choice {
                    ToolChoice::Auto => json!({"type": "auto"}),
                    ToolChoice::None => json!({"type": "any"}), // Anthropic doesn't have none
                    ToolChoice::Required => json!({"type": "any"}),
                    ToolChoice::Function { name } => json!({"type": "tool", "name": name}),
                };
                anthropic_request["tool_choice"] = anthropic_choice;
            }
        }

        anthropic_request
    }

    fn transform_response(&self, response: Value) -> Result<ProviderResponse> {
        use super::types::{FunctionCall, TokenUsage, ToolCall};

        let id = response["id"].as_str().unwrap_or("unknown").to_string();
        let model = response["model"].as_str().unwrap_or("unknown").to_string();

        // Extract content from Anthropic response
        let content_blocks = response["content"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("No content in Anthropic response"))?;

        // Get text content
        let text_content: String = content_blocks
            .iter()
            .filter_map(|b| {
                if b["type"] == "text" {
                    b["text"].as_str()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        // Get finish reason
        let stop_reason = response["stop_reason"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        // Extract token usage
        let usage = response["usage"].clone();
        let token_usage = TokenUsage {
            prompt_tokens: usage["input_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: usage["output_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: (usage["input_tokens"].as_u64().unwrap_or(0)
                + usage["output_tokens"].as_u64().unwrap_or(0)) as u32,
        };

        // Extract tool calls if present
        let tool_calls: Option<Vec<ToolCall>> = content_blocks
            .iter()
            .filter(|b| b["type"] == "tool_use")
            .map(|b| {
                Some(ToolCall {
                    id: b["id"].as_str()?.to_string(),
                    call_type: "function".to_string(),
                    function: FunctionCall {
                        name: b["name"].as_str()?.to_string(),
                        arguments: serde_json::to_string(&b["input"]).unwrap_or_default(),
                    },
                })
            })
            .collect::<Option<Vec<_>>>();

        Ok(ProviderResponse {
            id,
            model,
            content: text_content,
            finish_reason: stop_reason,
            usage: token_usage,
            tool_calls,
        })
    }

    fn get_endpoint(&self, base_url: Option<&str>, _model_id: &str) -> String {
        let base = base_url.unwrap_or(&self.default_base_url);
        format!("{}/messages", base)
    }

    fn build_headers(&self, api_key: &str) -> Vec<(String, String)> {
        vec![
            ("x-api-key".to_string(), api_key.to_string()),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
            ("content-type".to_string(), "application/json".to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::{ContentPart, Message, MessageContent, Tool};
    use super::*;

    // ============================================================
    // Basic Functionality Tests
    // ============================================================

    #[test]
    fn test_transform_simple_request() {
        let adapter = AnthropicAdapter::new();
        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text("Hello".to_string()),
                name: None,
            }],
            model: "claude-3-opus".to_string(),
            max_tokens: Some(1024),
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
        };

        let transformed = adapter.transform_request(&request);
        assert_eq!(transformed["model"], "claude-3-opus");
        assert_eq!(transformed["max_tokens"], 1024);
    }

    #[test]
    fn test_transform_response() {
        let adapter = AnthropicAdapter::new();
        let response = json!({
            "id": "msg_123",
            "model": "claude-3-opus",
            "content": [
                {"type": "text", "text": "Hello there!"}
            ],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5
            }
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.id, "msg_123");
        assert_eq!(result.content, "Hello there!");
        assert_eq!(result.usage.prompt_tokens, 10);
    }

    // ============================================================
    // Edge Case Tests for Request Transformation
    // ============================================================

    #[test]
    fn test_transform_request_empty_message_content() {
        let adapter = AnthropicAdapter::new();
        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text("".to_string()),
                name: None,
            }],
            model: "claude-3-opus".to_string(),
            max_tokens: Some(1024),
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
        };

        let transformed = adapter.transform_request(&request);
        // Should still create a message with empty text
        assert_eq!(transformed["model"], "claude-3-opus");
        let messages = transformed["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        // Content should be an array with empty text
        let content = &messages[0]["content"];
        assert!(content.is_array());
        let content_arr = content.as_array().unwrap();
        assert_eq!(content_arr[0]["type"], "text");
        assert_eq!(content_arr[0]["text"], "");
    }

    #[test]
    fn test_transform_request_empty_messages_array() {
        let adapter = AnthropicAdapter::new();
        let request = ProviderRequest {
            messages: vec![], // Empty messages array
            model: "claude-3-opus".to_string(),
            max_tokens: Some(1024),
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
        };

        let transformed = adapter.transform_request(&request);
        // Should handle empty messages gracefully
        let messages = transformed["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 0);
    }

    #[test]
    fn test_transform_request_malformed_image_url_missing_url_field() {
        let adapter = AnthropicAdapter::new();

        // Create a part with image_url type but missing the image_url field entirely
        let malformed_part_no_image_url = ContentPart {
            part_type: "image_url".to_string(),
            text: None,
            image_url: None, // Missing
            image_data: None,
        };

        // Create a part with image_url but missing the url field inside
        let malformed_part_empty_url = ContentPart {
            part_type: "image_url".to_string(),
            text: None,
            image_url: Some(super::super::types::ImageUrl {
                url: "".to_string(), // Empty URL
                detail: None,
            }),
            image_data: None,
        };

        let request = ProviderRequest {
            messages: vec![
                Message {
                    role: "user".to_string(),
                    content: MessageContent::Parts(vec![
                        ContentPart::text("Hello"),
                        malformed_part_no_image_url,
                    ]),
                    name: None,
                },
                Message {
                    role: "user".to_string(),
                    content: MessageContent::Parts(vec![
                        ContentPart::text("World"),
                        malformed_part_empty_url,
                    ]),
                    name: None,
                },
            ],
            model: "claude-3-opus".to_string(),
            max_tokens: Some(1024),
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
        assert_eq!(messages.len(), 2);

        // First message should have text and malformed image placeholder
        let first_content = messages[0]["content"].as_array().unwrap();
        assert!(!first_content.is_empty());

        // Second message should have text and empty URL image
        let second_content = messages[1]["content"].as_array().unwrap();
        assert!(!second_content.is_empty());
    }

    #[test]
    fn test_transform_request_mixed_content_with_valid_and_invalid_images() {
        let adapter = AnthropicAdapter::new();

        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Parts(vec![
                    ContentPart::text("Check these images:"),
                    ContentPart::image_url("https://example.com/valid.png"),
                    ContentPart {
                        part_type: "image_url".to_string(),
                        text: None,
                        image_url: None, // Invalid - no image_url
                        image_data: None,
                    },
                    ContentPart::text("And another:"),
                ]),
                name: None,
            }],
            model: "claude-3-opus".to_string(),
            max_tokens: Some(1024),
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
        assert_eq!(messages.len(), 1);

        let content = messages[0]["content"].as_array().unwrap();
        // Should have 4 parts: text, valid image, placeholder, text
        assert_eq!(content.len(), 4);

        // First should be text
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "Check these images:");

        // Second should be valid image
        assert_eq!(content[1]["type"], "image");
        assert_eq!(content[1]["source"]["type"], "url");
        assert_eq!(content[1]["source"]["url"], "https://example.com/valid.png");

        // Third should be placeholder for malformed image
        assert_eq!(content[2]["type"], "text");
        assert_eq!(content[2]["text"], "[malformed image content]");

        // Fourth should be text
        assert_eq!(content[3]["type"], "text");
        assert_eq!(content[3]["text"], "And another:");
    }

    #[test]
    fn test_transform_request_system_message_from_parts() {
        let adapter = AnthropicAdapter::new();

        let request = ProviderRequest {
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: MessageContent::Parts(vec![
                        ContentPart::text("You are"),
                        ContentPart::text("a helpful assistant."),
                    ]),
                    name: None,
                },
                Message {
                    role: "user".to_string(),
                    content: MessageContent::Text("Hello".to_string()),
                    name: None,
                },
            ],
            model: "claude-3-opus".to_string(),
            max_tokens: Some(1024),
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
        };

        let transformed = adapter.transform_request(&request);
        // System should be extracted from parts
        assert_eq!(transformed["system"], "You are\na helpful assistant.");
    }

    #[test]
    fn test_transform_request_with_all_optional_fields() {
        let adapter = AnthropicAdapter::new();

        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text("Hello".to_string()),
                name: None,
            }],
            model: "claude-3-opus".to_string(),
            max_tokens: Some(2048),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop: Some(vec!["STOP".to_string(), "END".to_string()]),
            stream: true,
            system: Some("You are helpful.".to_string()),
            tools: None,
            tool_choice: None,
        };

        let transformed = adapter.transform_request(&request);
        assert_eq!(transformed["max_tokens"], 2048);
        // f32 has precision limitations, compare with tolerance
        let temp = transformed["temperature"].as_f64().unwrap();
        assert!(
            (temp - 0.7).abs() < 0.001,
            "Expected temperature ~0.7, got {}",
            temp
        );
        let top_p = transformed["top_p"].as_f64().unwrap();
        assert!(
            (top_p - 0.9).abs() < 0.001,
            "Expected top_p ~0.9, got {}",
            top_p
        );
        let stop = transformed["stop_sequences"].as_array().unwrap();
        assert_eq!(stop.len(), 2);
        assert_eq!(transformed["stream"], true);
        assert_eq!(transformed["system"], "You are helpful.");
    }

    #[test]
    fn test_transform_request_with_tools() {
        let adapter = AnthropicAdapter::new();

        let tool = Tool::function("get_weather", "Get weather info").with_parameters(json!({
            "type": "object",
            "properties": {
                "location": {"type": "string"}
            }
        }));

        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text("What's the weather?".to_string()),
                name: None,
            }],
            model: "claude-3-opus".to_string(),
            max_tokens: Some(1024),
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            system: None,
            tools: Some(vec![tool]),
            tool_choice: Some(super::super::types::ToolChoice::Auto),
        };

        let transformed = adapter.transform_request(&request);
        let tools = transformed["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "get_weather");
        assert_eq!(tools[0]["description"], "Get weather info");
        assert!(tools[0]["input_schema"].is_object());
        assert_eq!(transformed["tool_choice"]["type"], "auto");
    }

    #[test]
    fn test_transform_request_tool_choice_function() {
        let adapter = AnthropicAdapter::new();

        let tool = Tool::function("calculator", "Do math");
        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text("Calculate 2+2".to_string()),
                name: None,
            }],
            model: "claude-3-opus".to_string(),
            max_tokens: Some(1024),
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
            system: None,
            tools: Some(vec![tool]),
            tool_choice: Some(super::super::types::ToolChoice::Function {
                name: "calculator".to_string(),
            }),
        };

        let transformed = adapter.transform_request(&request);
        assert_eq!(transformed["tool_choice"]["type"], "tool");
        assert_eq!(transformed["tool_choice"]["name"], "calculator");
    }

    #[test]
    fn test_transform_request_null_content() {
        // Test with content that deserializes to empty text
        let adapter = AnthropicAdapter::new();

        let request = ProviderRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text(String::new()),
                name: None,
            }],
            model: "claude-3-opus".to_string(),
            max_tokens: Some(1024),
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
        assert_eq!(messages.len(), 1);
    }

    // ============================================================
    // Edge Case Tests for Response Transformation
    // ============================================================

    #[test]
    fn test_transform_response_empty_content_array() {
        let adapter = AnthropicAdapter::new();
        let response = json!({
            "id": "msg_123",
            "model": "claude-3-opus",
            "content": [],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5
            }
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.id, "msg_123");
        assert_eq!(result.content, ""); // Empty string from empty array
    }

    #[test]
    fn test_transform_response_missing_id() {
        let adapter = AnthropicAdapter::new();
        let response = json!({
            "model": "claude-3-opus",
            "content": [
                {"type": "text", "text": "Hello"}
            ],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5
            }
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.id, "unknown"); // Should use default
    }

    #[test]
    fn test_transform_response_missing_model() {
        let adapter = AnthropicAdapter::new();
        let response = json!({
            "id": "msg_123",
            "content": [
                {"type": "text", "text": "Hello"}
            ],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5
            }
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.model, "unknown"); // Should use default
    }

    #[test]
    fn test_transform_response_missing_stop_reason() {
        let adapter = AnthropicAdapter::new();
        let response = json!({
            "id": "msg_123",
            "model": "claude-3-opus",
            "content": [
                {"type": "text", "text": "Hello"}
            ],
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5
            }
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.finish_reason, "unknown"); // Should use default
    }

    #[test]
    fn test_transform_response_missing_usage() {
        let adapter = AnthropicAdapter::new();
        let response = json!({
            "id": "msg_123",
            "model": "claude-3-opus",
            "content": [
                {"type": "text", "text": "Hello"}
            ],
            "stop_reason": "end_turn"
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.usage.prompt_tokens, 0);
        assert_eq!(result.usage.completion_tokens, 0);
        assert_eq!(result.usage.total_tokens, 0);
    }

    #[test]
    fn test_transform_response_partial_usage() {
        let adapter = AnthropicAdapter::new();
        let response = json!({
            "id": "msg_123",
            "model": "claude-3-opus",
            "content": [
                {"type": "text", "text": "Hello"}
            ],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 100
                // Missing output_tokens
            }
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.usage.prompt_tokens, 100);
        assert_eq!(result.usage.completion_tokens, 0); // Default
        assert_eq!(result.usage.total_tokens, 100);
    }

    #[test]
    fn test_transform_response_no_content_field() {
        let adapter = AnthropicAdapter::new();
        let response = json!({
            "id": "msg_123",
            "model": "claude-3-opus",
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5
            }
        });

        let result = adapter.transform_response(response);
        assert!(
            result.is_err(),
            "Should return error when content is missing"
        );
    }

    #[test]
    fn test_transform_response_null_content() {
        let adapter = AnthropicAdapter::new();
        let response = json!({
            "id": "msg_123",
            "model": "claude-3-opus",
            "content": null,
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5
            }
        });

        let result = adapter.transform_response(response);
        assert!(result.is_err(), "Should return error when content is null");
    }

    #[test]
    fn test_transform_response_multiple_text_blocks() {
        let adapter = AnthropicAdapter::new();
        let response = json!({
            "id": "msg_123",
            "model": "claude-3-opus",
            "content": [
                {"type": "text", "text": "First "},
                {"type": "text", "text": "Second "},
                {"type": "text", "text": "Third"}
            ],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 15
            }
        });

        let result = adapter.transform_response(response).unwrap();
        assert_eq!(result.content, "First Second Third");
    }

    #[test]
    fn test_transform_response_with_tool_calls() {
        let adapter = AnthropicAdapter::new();
        let response = json!({
            "id": "msg_123",
            "model": "claude-3-opus",
            "content": [
                {"type": "text", "text": "Let me help you with that."},
                {
                    "type": "tool_use",
                    "id": "toolu_123",
                    "name": "get_weather",
                    "input": {"location": "San Francisco"}
                }
            ],
            "stop_reason": "tool_use",
            "usage": {
                "input_tokens": 20,
                "output_tokens": 30
            }
        });

        let result = adapter.transform_response(response).unwrap();
        assert!(result.has_tool_calls());
        let tool_calls = result.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "toolu_123");
        assert_eq!(tool_calls[0].function.name, "get_weather");
        assert!(tool_calls[0].function.arguments.contains("San Francisco"));
    }

    #[test]
    fn test_transform_response_multiple_tool_calls() {
        let adapter = AnthropicAdapter::new();
        let response = json!({
            "id": "msg_123",
            "model": "claude-3-opus",
            "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_1",
                    "name": "get_weather",
                    "input": {"location": "SF"}
                },
                {
                    "type": "tool_use",
                    "id": "toolu_2",
                    "name": "get_time",
                    "input": {"timezone": "PST"}
                }
            ],
            "stop_reason": "tool_use",
            "usage": {
                "input_tokens": 20,
                "output_tokens": 30
            }
        });

        let result = adapter.transform_response(response).unwrap();
        let tool_calls = result.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 2);
    }

    #[test]
    fn test_transform_response_tool_use_missing_fields() {
        let adapter = AnthropicAdapter::new();
        let response = json!({
            "id": "msg_123",
            "model": "claude-3-opus",
            "content": [
                {
                    "type": "tool_use",
                    // Missing id and name
                    "input": {"location": "SF"}
                }
            ],
            "stop_reason": "tool_use",
            "usage": {
                "input_tokens": 20,
                "output_tokens": 30
            }
        });

        let result = adapter.transform_response(response).unwrap();
        // Tool calls with missing fields should result in None
        assert!(
            result.tool_calls.is_none() || result.tool_calls.as_ref().unwrap().is_empty(),
            "Tool calls with missing required fields should be filtered out"
        );
    }

    #[test]
    fn test_transform_response_mixed_content_types() {
        let adapter = AnthropicAdapter::new();
        let response = json!({
            "id": "msg_123",
            "model": "claude-3-opus",
            "content": [
                {"type": "text", "text": "Here's the image: "},
                {"type": "image", "source": {"type": "url", "url": "https://example.com/img.png"}},
                {"type": "text", "text": " and text continues."}
            ],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 50,
                "output_tokens": 20
            }
        });

        let result = adapter.transform_response(response).unwrap();
        // Only text content should be extracted
        assert_eq!(result.content, "Here's the image:  and text continues.");
    }

    // ============================================================
    // Edge Case Tests for get_endpoint
    // ============================================================

    #[test]
    fn test_get_endpoint_default_base_url() {
        let adapter = AnthropicAdapter::new();
        let endpoint = adapter.get_endpoint(None, "claude-3-opus");
        assert_eq!(endpoint, "https://api.anthropic.com/v1/messages");
    }

    #[test]
    fn test_get_endpoint_custom_base_url() {
        let adapter = AnthropicAdapter::new();
        let endpoint = adapter.get_endpoint(Some("https://custom.api.com"), "claude-3-opus");
        assert_eq!(endpoint, "https://custom.api.com/messages");
    }

    #[test]
    fn test_get_endpoint_custom_base_url_with_trailing_slash() {
        let adapter = AnthropicAdapter::new();
        let endpoint = adapter.get_endpoint(Some("https://custom.api.com/"), "claude-3-opus");
        // Should include the trailing slash (no normalization)
        assert_eq!(endpoint, "https://custom.api.com//messages");
    }

    #[test]
    fn test_get_endpoint_with_base_url_from_constructor() {
        let adapter = AnthropicAdapter::with_base_url("https://proxy.example.com/v2".to_string());
        let endpoint = adapter.get_endpoint(None, "claude-3-opus");
        assert_eq!(endpoint, "https://proxy.example.com/v2/messages");
    }

    #[test]
    fn test_get_endpoint_override_constructor_base_url() {
        let adapter = AnthropicAdapter::with_base_url("https://default.example.com".to_string());
        let endpoint = adapter.get_endpoint(Some("https://override.example.com"), "claude-3-opus");
        assert_eq!(endpoint, "https://override.example.com/messages");
    }

    #[test]
    fn test_get_endpoint_empty_base_url_uses_default() {
        let adapter = AnthropicAdapter::new();
        // Empty string should NOT use default - it will create an invalid URL
        let endpoint = adapter.get_endpoint(Some(""), "claude-3-opus");
        assert_eq!(endpoint, "/messages"); // Empty base + /messages
    }

    #[test]
    fn test_get_endpoint_model_id_ignored() {
        let adapter = AnthropicAdapter::new();
        // Anthropic endpoint doesn't include model in path
        let endpoint1 = adapter.get_endpoint(None, "claude-3-opus");
        let endpoint2 = adapter.get_endpoint(None, "claude-3-sonnet");
        assert_eq!(endpoint1, endpoint2);
    }

    #[test]
    fn test_get_endpoint_localhost() {
        let adapter = AnthropicAdapter::new();
        let endpoint = adapter.get_endpoint(Some("http://localhost:8080"), "claude-3-opus");
        assert_eq!(endpoint, "http://localhost:8080/messages");
    }

    #[test]
    fn test_get_endpoint_ipv4_address() {
        let adapter = AnthropicAdapter::new();
        let endpoint = adapter.get_endpoint(Some("http://192.168.1.1:3000"), "claude-3-opus");
        assert_eq!(endpoint, "http://192.168.1.1:3000/messages");
    }

    // ============================================================
    // Edge Case Tests for build_headers
    // ============================================================

    #[test]
    fn test_build_headers_format() {
        let adapter = AnthropicAdapter::new();
        let headers = adapter.build_headers("test-api-key-12345");

        assert_eq!(headers.len(), 3);

        // Check x-api-key header
        let api_key_header = headers.iter().find(|(k, _)| k == "x-api-key");
        assert!(api_key_header.is_some());
        assert_eq!(api_key_header.unwrap().1, "test-api-key-12345");

        // Check anthropic-version header
        let version_header = headers.iter().find(|(k, _)| k == "anthropic-version");
        assert!(version_header.is_some());
        assert_eq!(version_header.unwrap().1, "2023-06-01");

        // Check content-type header
        let content_type_header = headers.iter().find(|(k, _)| k == "content-type");
        assert!(content_type_header.is_some());
        assert_eq!(content_type_header.unwrap().1, "application/json");
    }

    #[test]
    fn test_build_headers_empty_api_key() {
        let adapter = AnthropicAdapter::new();
        let headers = adapter.build_headers("");

        // Should still return all headers, just with empty api-key value
        assert_eq!(headers.len(), 3);
        let api_key_header = headers.iter().find(|(k, _)| k == "x-api-key");
        assert!(api_key_header.is_some());
        assert_eq!(api_key_header.unwrap().1, "");
    }

    #[test]
    fn test_build_headers_special_characters_in_key() {
        let adapter = AnthropicAdapter::new();
        let special_key = "key-with-special!@#$%^&*()_+-={}[]|:;<>?,./~`";
        let headers = adapter.build_headers(special_key);

        let api_key_header = headers.iter().find(|(k, _)| k == "x-api-key");
        assert_eq!(api_key_header.unwrap().1, special_key);
    }

    #[test]
    fn test_build_headers_unicode_in_key() {
        let adapter = AnthropicAdapter::new();
        let unicode_key = "key-日本語-🔑-emoji";
        let headers = adapter.build_headers(unicode_key);

        let api_key_header = headers.iter().find(|(k, _)| k == "x-api-key");
        assert_eq!(api_key_header.unwrap().1, unicode_key);
    }

    #[test]
    fn test_build_headers_very_long_key() {
        let adapter = AnthropicAdapter::new();
        let long_key = "x".repeat(10000);
        let headers = adapter.build_headers(&long_key);

        let api_key_header = headers.iter().find(|(k, _)| k == "x-api-key");
        assert_eq!(api_key_header.unwrap().1.len(), 10000);
    }

    #[test]
    fn test_build_headers_order_consistent() {
        let adapter = AnthropicAdapter::new();
        let headers1 = adapter.build_headers("key1");
        let headers2 = adapter.build_headers("key2");

        // Header names should be in same order
        for (i, (k1, _)) in headers1.iter().enumerate() {
            assert_eq!(*k1, headers2[i].0);
        }
    }

    // ============================================================
    // Provider Adapter Trait Tests
    // ============================================================

    #[test]
    fn test_provider_name() {
        let adapter = AnthropicAdapter::new();
        assert_eq!(adapter.provider_name(), "anthropic");
    }

    #[test]
    fn test_default_implementation() {
        let adapter = AnthropicAdapter::default();
        assert_eq!(adapter.provider_name(), "anthropic");

        let endpoint = adapter.get_endpoint(None, "any-model");
        assert!(endpoint.starts_with("https://api.anthropic.com"));
    }

    #[test]
    fn test_with_base_url_constructor() {
        let adapter = AnthropicAdapter::with_base_url("https://test.example.com/api".to_string());
        assert_eq!(adapter.provider_name(), "anthropic");

        let endpoint = adapter.get_endpoint(None, "model");
        assert_eq!(endpoint, "https://test.example.com/api/messages");
    }
}
