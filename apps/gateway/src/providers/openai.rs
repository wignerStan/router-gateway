//! OpenAI API adapter
//!
//! Transforms between gateway format and OpenAI's Chat Completions API.

use super::types::{ProviderAdapter, ProviderRequest, ProviderResponse};
use anyhow::Result;
use serde_json::{json, Value};

/// OpenAI API adapter
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: String) -> Self {
        Self {
            default_base_url: base_url,
        }
    }
}

impl ProviderAdapter for OpenAIAdapter {
    fn provider_name(&self) -> &str {
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
                                    "text": p.text
                                })
                            } else if p.part_type == "image_url" {
                                json!({
                                    "type": "image_url",
                                    "image_url": {
                                        "url": p.image_url.as_ref().map(|u| &u.url).unwrap_or(&String::new()),
                                        "detail": p.image_url.as_ref().and_then(|u| u.detail.clone())
                                    }
                                })
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
                            "parameters": t.function.parameters.clone().unwrap_or(json!({}))
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
            prompt_tokens: usage["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: usage["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: usage["total_tokens"].as_u64().unwrap_or(0) as u32,
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
        let base = base_url.unwrap_or(&self.default_base_url);
        format!("{}/chat/completions", base)
    }

    fn build_headers(&self, api_key: &str) -> Vec<(String, String)> {
        vec![
            ("Authorization".to_string(), format!("Bearer {}", api_key)),
            ("Content-Type".to_string(), "application/json".to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_transform_simple_request() {
        let adapter = OpenAIAdapter::new();
        let request = ProviderRequest {
            messages: vec![super::super::types::Message {
                role: "user".to_string(),
                content: super::super::types::MessageContent::Text("Hello".to_string()),
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
            "Expected temperature ~0.7, got {}",
            temp
        );
    }

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
}
