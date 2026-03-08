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
                    }
                    MessageContent::Parts(parts) => {
                        let texts: Vec<Value> = parts
                            .iter()
                            .filter_map(|p| {
                                p.text.as_ref().map(|t| json!({"text": t}))
                            })
                            .collect();
                        if !texts.is_empty() {
                            system_instruction = Some(json!({"parts": texts}));
                        }
                    }
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
                }
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
                }
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
        if let Some(sys) = system_instruction.or(request.system.as_ref().map(|s| {
            json!({"parts": [{"text": s}]})
        })) {
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
        use super::types::{TokenUsage, ToolCall, FunctionCall};

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
        let tool_calls: Option<Vec<ToolCall>> = first_candidate["content"]["parts"]
            .as_array()
            .map(|parts| {
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
            model: response.get("modelVersion")
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
        let base = base_url.unwrap_or(&self.default_base_url);
        format!("{}/models/{}:generateContent", base, model_id)
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
    use super::*;

    #[test]
    fn test_transform_simple_request() {
        let adapter = GoogleAdapter::new();
        let request = ProviderRequest {
            messages: vec![
                super::super::types::Message {
                    role: "user".to_string(),
                    content: super::super::types::MessageContent::Text("Hello".to_string()),
                    name: None,
                }
            ],
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
}
