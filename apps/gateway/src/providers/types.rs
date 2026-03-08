//! Shared types for provider adapters
//!
//! Defines the normalized request/response types that all adapters use.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Normalized request format
#[derive(Debug, Clone)]
pub struct ProviderRequest {
    /// Messages in normalized format
    pub messages: Vec<Message>,
    /// Model identifier
    pub model: String,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Temperature (0.0 - 2.0)
    pub temperature: Option<f32>,
    /// Top-p sampling
    pub top_p: Option<f32>,
    /// Stop sequences
    pub stop: Option<Vec<String>>,
    /// Enable streaming
    pub stream: bool,
    /// System prompt override
    pub system: Option<String>,
    /// Tools/functions available
    pub tools: Option<Vec<Tool>>,
    /// Tool choice strategy
    pub tool_choice: Option<ToolChoice>,
}

impl ProviderRequest {
    /// Create a new request builder
    pub fn builder() -> ProviderRequestBuilder {
        ProviderRequestBuilder::default()
    }
}

/// Builder for ProviderRequest
#[derive(Default)]
pub struct ProviderRequestBuilder {
    messages: Vec<Message>,
    model: Option<String>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    stop: Option<Vec<String>>,
    stream: bool,
    system: Option<String>,
    tools: Option<Vec<Tool>>,
    tool_choice: Option<ToolChoice>,
}

impl ProviderRequestBuilder {
    pub fn messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }

    pub fn system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    pub fn tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn build(self) -> Result<ProviderRequest, String> {
        let model = self.model.ok_or("model is required")?;
        Ok(ProviderRequest {
            messages: self.messages,
            model,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            top_p: self.top_p,
            stop: self.stop,
            stream: self.stream,
            system: self.system,
            tools: self.tools,
            tool_choice: self.tool_choice,
        })
    }
}

/// Message in normalized format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role: system, user, assistant, or tool
    pub role: String,
    /// Message content
    pub content: MessageContent,
    /// Name (for tool messages)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Message content (text or multi-modal)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Simple text content
    Text(String),
    /// Multi-part content
    Parts(Vec<ContentPart>),
}

impl MessageContent {
    /// Create text content
    pub fn text(text: impl Into<String>) -> Self {
        MessageContent::Text(text.into())
    }

    /// Get text if this is text-only content
    pub fn as_text(&self) -> Option<&str> {
        match self {
            MessageContent::Text(t) => Some(t),
            MessageContent::Parts(_) => None,
        }
    }
}

/// Content part for multi-modal messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPart {
    /// Part type: text, image_url, image, etc.
    #[serde(rename = "type")]
    pub part_type: String,
    /// Text content (for text parts)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Image URL (for image_url parts)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<ImageUrl>,
    /// Image data (for inline images)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_data: Option<ImageData>,
}

impl ContentPart {
    /// Create a text part
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            part_type: "text".to_string(),
            text: Some(text.into()),
            image_url: None,
            image_data: None,
        }
    }

    /// Create an image URL part
    pub fn image_url(url: impl Into<String>) -> Self {
        Self {
            part_type: "image_url".to_string(),
            text: None,
            image_url: Some(ImageUrl {
                url: url.into(),
                detail: None,
            }),
            image_data: None,
        }
    }
}

/// Image URL structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    /// The URL to the image
    pub url: String,
    /// Detail level: low, high, auto
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Inline image data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    /// MIME type
    pub mime_type: String,
    /// Base64-encoded data
    pub data: String,
}

/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// Tool type (usually "function")
    #[serde(rename = "type")]
    pub tool_type: String,
    /// Function definition
    pub function: FunctionDef,
}

impl Tool {
    /// Create a function tool
    pub fn function(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: name.into(),
                description: Some(description.into()),
                parameters: None,
            },
        }
    }

    /// Add parameters schema
    pub fn with_parameters(mut self, parameters: Value) -> Self {
        self.function.parameters = Some(parameters);
        self
    }
}

/// Function definition for tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDef {
    /// Function name
    pub name: String,
    /// Function description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Parameters schema (JSON Schema)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
}

/// Tool choice strategy
#[derive(Debug, Clone)]
pub enum ToolChoice {
    /// Let the model decide
    Auto,
    /// Don't use tools
    None,
    /// Must use a tool
    Required,
    /// Use a specific function
    Function { name: String },
}

/// Normalized response format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderResponse {
    /// Response ID from provider
    pub id: String,
    /// Model used
    pub model: String,
    /// Generated content
    pub content: String,
    /// Finish reason
    pub finish_reason: String,
    /// Token usage
    pub usage: TokenUsage,
    /// Tool calls if any
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl ProviderResponse {
    /// Check if this is a tool call response
    pub fn has_tool_calls(&self) -> bool {
        self.tool_calls.as_ref().map(|t| !t.is_empty()).unwrap_or(false)
    }
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    /// Prompt tokens used
    pub prompt_tokens: u32,
    /// Completion tokens generated
    pub completion_tokens: u32,
    /// Total tokens
    pub total_tokens: u32,
}

impl TokenUsage {
    /// Create new token usage
    pub fn new(prompt: u32, completion: u32) -> Self {
        Self {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: prompt + completion,
        }
    }
}

/// Tool call in response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Call ID
    pub id: String,
    /// Call type (function)
    #[serde(rename = "type")]
    pub call_type: String,
    /// Function call details
    pub function: FunctionCall,
}

/// Function call details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// Function name
    pub name: String,
    /// Arguments as JSON string
    pub arguments: String,
}

/// Provider error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderError {
    /// Error type
    #[serde(rename = "type")]
    pub error_type: String,
    /// Error message
    pub message: String,
    /// Error code
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.error_type, self.message)
    }
}

impl std::error::Error for ProviderError {}

/// Trait for provider-specific adapters
pub trait ProviderAdapter: Send + Sync {
    /// Get the provider name
    fn provider_name(&self) -> &str;

    /// Transform a normalized request to provider-specific format
    fn transform_request(&self, request: &ProviderRequest) -> Value;

    /// Transform a provider-specific response to normalized format
    fn transform_response(&self, response: Value) -> anyhow::Result<ProviderResponse>;

    /// Get the API endpoint URL for a model
    fn get_endpoint(&self, base_url: Option<&str>, model_id: &str) -> String;

    /// Build headers for the API request
    fn build_headers(&self, api_key: &str) -> Vec<(String, String)>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_request_builder() {
        let request = ProviderRequest::builder()
            .model("gpt-4")
            .messages(vec![Message {
                role: "user".to_string(),
                content: MessageContent::text("Hello"),
                name: None,
            }])
            .temperature(0.7)
            .max_tokens(1024)
            .build()
            .unwrap();

        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.max_tokens, Some(1024));
    }

    #[test]
    fn test_message_content() {
        let text = MessageContent::text("Hello");
        assert_eq!(text.as_text(), Some("Hello"));

        let parts = MessageContent::Parts(vec![
            ContentPart::text("Part 1"),
        ]);
        assert!(parts.as_text().is_none());
    }

    #[test]
    fn test_tool_builder() {
        let tool = Tool::function("get_weather", "Get current weather")
            .with_parameters(json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                }
            }));

        assert_eq!(tool.function.name, "get_weather");
        assert!(tool.function.parameters.is_some());
    }
}
