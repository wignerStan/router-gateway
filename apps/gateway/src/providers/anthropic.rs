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
    use super::*;

    #[test]
    fn test_transform_simple_request() {
        let adapter = AnthropicAdapter::new();
        let request = ProviderRequest {
            messages: vec![super::super::types::Message {
                role: "user".to_string(),
                content: super::super::types::MessageContent::Text("Hello".to_string()),
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
}
