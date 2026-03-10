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
        self.tool_calls
            .as_ref()
            .map(|t| !t.is_empty())
            .unwrap_or(false)
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

        let parts = MessageContent::Parts(vec![ContentPart::text("Part 1")]);
        assert!(parts.as_text().is_none());
    }

    #[test]
    fn test_tool_builder() {
        let tool = Tool::function("get_weather", "Get current weather").with_parameters(json!({
            "type": "object",
            "properties": {
                "location": {"type": "string"}
            }
        }));

        assert_eq!(tool.function.name, "get_weather");
        assert!(tool.function.parameters.is_some());
    }

    // ============================================================
    // ContentPart Tests
    // ============================================================

    #[test]
    fn test_content_part_text() {
        let part = ContentPart::text("Hello world");
        assert_eq!(part.part_type, "text");
        assert_eq!(part.text, Some("Hello world".to_string()));
        assert!(part.image_url.is_none());
        assert!(part.image_data.is_none());
    }

    #[test]
    fn test_content_part_image_url() {
        let part = ContentPart::image_url("https://example.com/image.png");
        assert_eq!(part.part_type, "image_url");
        assert!(part.text.is_none());
        assert!(part.image_url.is_some());
        assert_eq!(
            part.image_url.as_ref().unwrap().url,
            "https://example.com/image.png"
        );
        assert!(part.image_url.as_ref().unwrap().detail.is_none());
        assert!(part.image_data.is_none());
    }

    #[test]
    fn test_content_part_serialization() {
        let part = ContentPart::text("Test");
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"Test\""));
    }

    #[test]
    fn test_content_part_deserialization() {
        let json = json!({
            "type": "text",
            "text": "Hello"
        });
        let part: ContentPart = serde_json::from_value(json).unwrap();
        assert_eq!(part.part_type, "text");
        assert_eq!(part.text, Some("Hello".to_string()));
    }

    #[test]
    fn test_content_part_with_image_data() {
        let part = ContentPart {
            part_type: "image".to_string(),
            text: None,
            image_url: None,
            image_data: Some(ImageData {
                mime_type: "image/png".to_string(),
                data: "base64encoded".to_string(),
            }),
        };
        assert_eq!(part.part_type, "image");
        assert!(part.image_data.is_some());
        let img_data = part.image_data.unwrap();
        assert_eq!(img_data.mime_type, "image/png");
        assert_eq!(img_data.data, "base64encoded");
    }

    // ============================================================
    // ImageUrl Tests
    // ============================================================

    #[test]
    fn test_image_url_with_detail() {
        let url = ImageUrl {
            url: "https://example.com/img.png".to_string(),
            detail: Some("high".to_string()),
        };
        assert_eq!(url.url, "https://example.com/img.png");
        assert_eq!(url.detail, Some("high".to_string()));
    }

    #[test]
    fn test_image_url_serialization() {
        let url = ImageUrl {
            url: "https://example.com/img.png".to_string(),
            detail: Some("auto".to_string()),
        };
        let json = serde_json::to_string(&url).unwrap();
        assert!(json.contains("\"url\":\"https://example.com/img.png\""));
        assert!(json.contains("\"detail\":\"auto\""));
    }

    #[test]
    fn test_image_url_deserialization() {
        let json = json!({
            "url": "https://example.com/img.png"
        });
        let url: ImageUrl = serde_json::from_value(json).unwrap();
        assert_eq!(url.url, "https://example.com/img.png");
        assert!(url.detail.is_none());
    }

    // ============================================================
    // ImageData Tests
    // ============================================================

    #[test]
    fn test_image_data() {
        let data = ImageData {
            mime_type: "image/jpeg".to_string(),
            data: "base64data".to_string(),
        };
        assert_eq!(data.mime_type, "image/jpeg");
        assert_eq!(data.data, "base64data");
    }

    #[test]
    fn test_image_data_serialization() {
        let data = ImageData {
            mime_type: "image/png".to_string(),
            data: "abc123".to_string(),
        };
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"mime_type\":\"image/png\""));
        assert!(json.contains("\"data\":\"abc123\""));
    }

    // ============================================================
    // Tool Tests
    // ============================================================

    #[test]
    fn test_tool_function_without_parameters() {
        let tool = Tool::function("test", "A test function");
        assert_eq!(tool.tool_type, "function");
        assert_eq!(tool.function.name, "test");
        assert_eq!(
            tool.function.description,
            Some("A test function".to_string())
        );
        assert!(tool.function.parameters.is_none());
    }

    #[test]
    fn test_tool_serialization() {
        let tool = Tool::function("calc", "Calculator").with_parameters(json!({"type": "object"}));
        let json_str = serde_json::to_string(&tool).unwrap();
        assert!(json_str.contains("\"type\":\"function\""));
        assert!(json_str.contains("\"name\":\"calc\""));
    }

    // ============================================================
    // MessageContent Tests
    // ============================================================

    #[test]
    fn test_message_content_parts() {
        let content = MessageContent::Parts(vec![
            ContentPart::text("Hello"),
            ContentPart::image_url("https://example.com/img.png"),
        ]);
        match content {
            MessageContent::Parts(parts) => {
                assert_eq!(parts.len(), 2);
                assert_eq!(parts[0].part_type, "text");
                assert_eq!(parts[1].part_type, "image_url");
            },
            _ => panic!("Expected Parts variant"),
        }
    }

    #[test]
    fn test_message_content_text_variant() {
        let content = MessageContent::Text("Hello world".to_string());
        match content {
            MessageContent::Text(text) => assert_eq!(text, "Hello world"),
            _ => panic!("Expected Text variant"),
        }
    }

    // ============================================================
    // FunctionDef Tests
    // ============================================================

    #[test]
    fn test_function_def() {
        let func = FunctionDef {
            name: "test_func".to_string(),
            description: Some("A test function".to_string()),
            parameters: Some(json!({"type": "object"})),
        };
        assert_eq!(func.name, "test_func");
        assert_eq!(func.description, Some("A test function".to_string()));
        assert!(func.parameters.is_some());
    }

    #[test]
    fn test_function_def_serialization() {
        let func = FunctionDef {
            name: "my_func".to_string(),
            description: None,
            parameters: None,
        };
        let json = serde_json::to_string(&func).unwrap();
        assert!(json.contains("\"name\":\"my_func\""));
        // description and parameters should be omitted when None
        assert!(!json.contains("\"description\""));
        assert!(!json.contains("\"parameters\""));
    }

    // ============================================================
    // Message Tests
    // ============================================================

    #[test]
    fn test_message() {
        let msg = Message {
            role: "user".to_string(),
            content: MessageContent::text("Hello"),
            name: Some("Alice".to_string()),
        };
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content.as_text(), Some("Hello"));
        assert_eq!(msg.name, Some("Alice".to_string()));
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message {
            role: "assistant".to_string(),
            content: MessageContent::Text("Hi there".to_string()),
            name: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"assistant\""));
        assert!(json.contains("\"content\":\"Hi there\""));
    }
}
