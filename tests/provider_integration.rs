#![allow(
    clippy::unreadable_literal,
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used
)]
// Integration tests for provider adapters
//
// Tests transform_request, transform_response, get_endpoint, and build_headers
// for OpenAI, Anthropic, and Google adapters using insta snapshot testing.
// Full HTTP round-trip tests (wiremock) are included but #[ignore]d until
// the HTTP execution layer is implemented.

#[cfg(test)]
mod openai {
    mod success {
        use gateway::providers::openai::OpenAIAdapter;
        use gateway::providers::types::ProviderAdapter;
        use serde_json::json;

        #[test]
        fn transform_response_chat_completion() {
            let adapter = OpenAIAdapter::new();
            let response = json!({
                "id": "chatcmpl-9bZ3T2z1kQwX5vNpM8rL",
                "object": "chat.completion",
                "created": 1710000000,
                "model": "gpt-4-0613",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello! How can I help you today?"
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 8,
                    "total_tokens": 18
                }
            });

            let result = adapter.transform_response(response).unwrap();
            insta::assert_json_snapshot!("openai_success_chat_completion", result);
        }

        #[test]
        fn transform_response_with_tool_calls() {
            let adapter = OpenAIAdapter::new();
            let response = json!({
                "id": "chatcmpl-9cD4W3mNxRqY6wOqN9sM",
                "object": "chat.completion",
                "created": 1710000100,
                "model": "gpt-4-0613",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [{
                            "id": "call_abc123def456",
                            "type": "function",
                            "function": {
                                "name": "get_weather",
                                "arguments": "{\"location\": \"San Francisco, CA\", \"unit\": \"celsius\"}"
                            }
                        }]
                    },
                    "finish_reason": "tool_calls"
                }],
                "usage": {
                    "prompt_tokens": 25,
                    "completion_tokens": 15,
                    "total_tokens": 40
                }
            });

            let result = adapter.transform_response(response).unwrap();
            insta::assert_json_snapshot!("openai_success_tool_calls", result);
        }

        #[test]
        fn transform_response_multiple_choices() {
            let adapter = OpenAIAdapter::new();
            let response = json!({
                "id": "chatcmpl-multi-choice",
                "object": "chat.completion",
                "created": 1710000200,
                "model": "gpt-4",
                "choices": [
                    {
                        "index": 0,
                        "message": {"role": "assistant", "content": "First choice"},
                        "finish_reason": "stop"
                    },
                    {
                        "index": 1,
                        "message": {"role": "assistant", "content": "Second choice"},
                        "finish_reason": "stop"
                    }
                ],
                "usage": {
                    "prompt_tokens": 5,
                    "completion_tokens": 4,
                    "total_tokens": 9
                }
            });

            // Should extract the first choice
            let result = adapter.transform_response(response).unwrap();
            assert_eq!(result.content, "First choice");
        }
    }

    mod errors {
        use gateway::providers::openai::OpenAIAdapter;
        use gateway::providers::types::ProviderAdapter;
        use serde_json::json;

        #[test]
        fn transform_response_missing_choices_returns_error() {
            let adapter = OpenAIAdapter::new();
            let response = json!({
                "error": {
                    "message": "Rate limit exceeded",
                    "type": "rate_limit_error",
                    "code": "429"
                }
            });
            let result = adapter.transform_response(response);
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(
                err.contains("No choices"),
                "Expected 'No choices' error, got: {err}"
            );
        }

        #[test]
        fn transform_response_empty_choices_returns_error() {
            let adapter = OpenAIAdapter::new();
            let response = json!({
                "id": "chatcmpl-err",
                "choices": [],
                "usage": {}
            });
            let result = adapter.transform_response(response);
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(
                err.contains("Empty choices"),
                "Expected 'Empty choices' error, got: {err}"
            );
        }

        #[test]
        fn transform_response_null_body_returns_error() {
            let adapter = OpenAIAdapter::new();
            let response = json!(null);
            let result = adapter.transform_response(response);
            assert!(result.is_err());
        }

        #[test]
        fn transform_response_missing_all_optional_fields() {
            let adapter = OpenAIAdapter::new();
            // Minimal valid response: choices present but everything else missing
            let response = json!({
                "choices": [{"message": {}}]
            });
            let result = adapter.transform_response(response).unwrap();
            assert_eq!(result.id, "unknown");
            assert_eq!(result.model, "unknown");
            assert_eq!(result.content, "");
            assert_eq!(result.finish_reason, "unknown");
            assert_eq!(result.usage.prompt_tokens, 0);
            assert_eq!(result.usage.completion_tokens, 0);
            assert_eq!(result.usage.total_tokens, 0);
        }
    }

    mod request_transform {
        use gateway::providers::openai::OpenAIAdapter;
        use gateway::providers::types::{
            ContentPart, ImageUrl, Message, MessageContent, ProviderAdapter, ProviderRequest, Tool,
            ToolChoice,
        };
        use rstest::rstest;
        use serde_json::json;

        #[test]
        fn simple_text_request() {
            let adapter = OpenAIAdapter::new();
            let request = ProviderRequest {
                messages: vec![Message {
                    role: "user".to_string(),
                    content: MessageContent::Text("Hello, GPT-4!".to_string()),
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
            insta::assert_json_snapshot!("openai_request_simple_text", transformed);
        }

        #[test]
        fn request_with_system_prompt() {
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
                system: Some("You are a helpful assistant.".to_string()),
                tools: None,
                tool_choice: None,
            };

            let transformed = adapter.transform_request(&request);
            insta::assert_json_snapshot!("openai_request_with_system", transformed);
        }

        #[test]
        fn request_with_tools_and_tool_choice() {
            let adapter = OpenAIAdapter::new();
            let request = ProviderRequest {
                messages: vec![Message {
                    role: "user".to_string(),
                    content: MessageContent::Text("What is the weather in Tokyo?".to_string()),
                    name: None,
                }],
                model: "gpt-4".to_string(),
                max_tokens: Some(2048),
                temperature: None,
                top_p: None,
                stop: None,
                stream: false,
                system: None,
                tools: Some(vec![Tool {
                    tool_type: "function".to_string(),
                    function: gateway::providers::types::FunctionDef {
                        name: "get_weather".to_string(),
                        description: Some("Get current weather for a location".to_string()),
                        parameters: Some(json!({
                            "type": "object",
                            "properties": {
                                "location": {
                                    "type": "string",
                                    "description": "City name"
                                },
                                "unit": {
                                    "type": "string",
                                    "enum": ["celsius", "fahrenheit"]
                                }
                            },
                            "required": ["location"]
                        })),
                    },
                }]),
                tool_choice: Some(ToolChoice::Auto),
            };

            let transformed = adapter.transform_request(&request);
            insta::assert_json_snapshot!("openai_request_with_tools", transformed);
        }

        #[test]
        fn request_with_vision_content() {
            let adapter = OpenAIAdapter::new();
            let request = ProviderRequest {
                messages: vec![Message {
                    role: "user".to_string(),
                    content: MessageContent::Parts(vec![
                        ContentPart {
                            part_type: "text".to_string(),
                            text: Some("Describe what you see in this image.".to_string()),
                            image_url: None,
                            image_data: None,
                        },
                        ContentPart {
                            part_type: "image_url".to_string(),
                            text: None,
                            image_url: Some(ImageUrl {
                                url: "https://example.com/photo.jpg".to_string(),
                                detail: Some("high".to_string()),
                            }),
                            image_data: None,
                        },
                    ]),
                    name: None,
                }],
                model: "gpt-4-vision-preview".to_string(),
                max_tokens: Some(4096),
                temperature: None,
                top_p: None,
                stop: None,
                stream: false,
                system: None,
                tools: None,
                tool_choice: None,
            };

            let transformed = adapter.transform_request(&request);
            insta::assert_json_snapshot!("openai_request_vision", transformed);
        }

        #[test]
        fn request_with_streaming_and_stop_sequences() {
            let adapter = OpenAIAdapter::new();
            let request = ProviderRequest {
                messages: vec![Message {
                    role: "user".to_string(),
                    content: MessageContent::Text("Generate a list".to_string()),
                    name: None,
                }],
                model: "gpt-4".to_string(),
                max_tokens: Some(512),
                temperature: Some(0.5),
                top_p: Some(0.9),
                stop: Some(vec!["\n\n".to_string(), "END".to_string()]),
                stream: true,
                system: None,
                tools: None,
                tool_choice: None,
            };

            let transformed = adapter.transform_request(&request);
            insta::assert_json_snapshot!("openai_request_streaming_stop", transformed);
        }

        #[rstest]
        #[case::default(None, "https://api.openai.com/v1/chat/completions")]
        #[case::custom(
            Some("https://proxy.example.com/v2"),
            "https://proxy.example.com/v2/chat/completions"
        )]
        #[case::trailing_slash(
            Some("https://proxy.example.com/v2/"),
            "https://proxy.example.com/v2/chat/completions"
        )]
        #[case::empty_string(Some(""), "https://api.openai.com/v1/chat/completions")]
        #[case::localhost(
            Some("http://localhost:8080"),
            "http://localhost:8080/chat/completions"
        )]
        fn get_endpoint(#[case] base_url: Option<&str>, #[case] expected: &str) {
            let adapter = OpenAIAdapter::new();
            assert_eq!(adapter.get_endpoint(base_url, "gpt-4"), expected);
        }

        #[test]
        fn build_headers_includes_bearer_auth() {
            let adapter = OpenAIAdapter::new();
            let headers = adapter.build_headers("sk-test-key-12345");
            assert_eq!(headers.len(), 2);

            let auth = headers.iter().find(|(k, _)| k == "Authorization");
            assert!(auth.is_some());
            assert_eq!(auth.unwrap().1, "Bearer sk-test-key-12345");

            let content_type = headers.iter().find(|(k, _)| k == "Content-Type");
            assert!(content_type.is_some());
            assert_eq!(content_type.unwrap().1, "application/json");
        }
    }

    // =========================================================================
    // Full HTTP round-trip tests (wiremock)
    //
    // These tests are #[ignore]d until the HTTP execution layer is implemented.
    // When the execution layer at src/routes.rs:449 is ready, remove the
    // #[ignore] attribute and these tests will validate end-to-end flows.
    // =========================================================================

    mod wiremock_round_trip {
        use gateway::providers::openai::OpenAIAdapter;
        use gateway::providers::types::{
            Message, MessageContent, ProviderAdapter, ProviderRequest,
        };
        use serde_json::json;

        #[tokio::test]
        #[ignore = "TODO: requires HTTP execution layer (see src/routes.rs:449)"]
        async fn openai_full_round_trip() {
            let mock_server = wiremock::MockServer::start().await;
            let adapter = OpenAIAdapter::with_base_url(mock_server.uri());

            wiremock::Mock::given(wiremock::matchers::method("POST"))
                .and(wiremock::matchers::path("/chat/completions"))
                .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                    "id": "chatcmpl-wiremock",
                    "object": "chat.completion",
                    "model": "gpt-4",
                    "choices": [{
                        "message": {
                            "role": "assistant",
                            "content": "Wiremock response"
                        },
                        "finish_reason": "stop"
                    }],
                    "usage": {
                        "prompt_tokens": 5,
                        "completion_tokens": 3,
                        "total_tokens": 8
                    }
                })))
                .mount(&mock_server)
                .await;

            let request = ProviderRequest {
                messages: vec![Message {
                    role: "user".to_string(),
                    content: MessageContent::Text("Hello via wiremock".to_string()),
                    name: None,
                }],
                model: "gpt-4".to_string(),
                max_tokens: Some(256),
                temperature: None,
                top_p: None,
                stop: None,
                stream: false,
                system: None,
                tools: None,
                tool_choice: None,
            };

            let endpoint = adapter.get_endpoint(None, "gpt-4");
            let _body = adapter.transform_request(&request);
            let _headers = adapter.build_headers("sk-test");

            // TODO: once HTTP execution layer exists, send request to endpoint
            // and verify transform_response produces expected ProviderResponse.
            let _ = (endpoint,);
        }

        #[tokio::test]
        #[ignore = "TODO: requires HTTP execution layer"]
        async fn openai_rate_limit_response() {
            let mock_server = wiremock::MockServer::start().await;
            let adapter = OpenAIAdapter::with_base_url(mock_server.uri());

            wiremock::Mock::given(wiremock::matchers::method("POST"))
                .respond_with(wiremock::ResponseTemplate::new(429).set_body_json(json!({
                    "error": {
                        "message": "Rate limit reached",
                        "type": "rate_limit_error",
                        "code": "429"
                    }
                })))
                .mount(&mock_server)
                .await;

            let endpoint = adapter.get_endpoint(None, "gpt-4");
            let _ = endpoint;
            // TODO: verify 429 handling when HTTP layer is ready
        }

        #[tokio::test]
        #[ignore = "TODO: requires HTTP execution layer"]
        async fn openai_auth_error_response() {
            let mock_server = wiremock::MockServer::start().await;
            let adapter = OpenAIAdapter::with_base_url(mock_server.uri());

            wiremock::Mock::given(wiremock::matchers::method("POST"))
                .respond_with(wiremock::ResponseTemplate::new(401).set_body_json(json!({
                    "error": {
                        "message": "Invalid API key",
                        "type": "authentication_error",
                        "code": "invalid_api_key"
                    }
                })))
                .mount(&mock_server)
                .await;

            let endpoint = adapter.get_endpoint(None, "gpt-4");
            let _ = endpoint;
            // TODO: verify 401 handling when HTTP layer is ready
        }

        #[tokio::test]
        #[ignore = "TODO: requires HTTP execution layer"]
        async fn openai_server_error_response() {
            let mock_server = wiremock::MockServer::start().await;
            let adapter = OpenAIAdapter::with_base_url(mock_server.uri());

            wiremock::Mock::given(wiremock::matchers::method("POST"))
                .respond_with(wiremock::ResponseTemplate::new(500).set_body_json(json!({
                    "error": {
                        "message": "Internal server error",
                        "type": "server_error",
                        "code": "internal_error"
                    }
                })))
                .mount(&mock_server)
                .await;

            let endpoint = adapter.get_endpoint(None, "gpt-4");
            let _ = endpoint;
            // TODO: verify 500 handling when HTTP layer is ready
        }
    }
}

#[cfg(test)]
mod anthropic {
    mod success {
        use gateway::providers::anthropic::AnthropicAdapter;
        use gateway::providers::types::ProviderAdapter;
        use serde_json::json;

        #[test]
        fn transform_response_message() {
            let adapter = AnthropicAdapter::new();
            let response = json!({
                "id": "msg_01XFDUDYJgAACzvnptvVoYEL",
                "type": "message",
                "role": "assistant",
                "content": [
                    {"type": "text", "text": "Hello! I'm Claude, how can I assist you?"}
                ],
                "model": "claude-3-opus-20240229",
                "stop_reason": "end_turn",
                "stop_sequence": null,
                "usage": {
                    "input_tokens": 10,
                    "output_tokens": 12
                }
            });

            let result = adapter.transform_response(response).unwrap();
            insta::assert_json_snapshot!("anthropic_success_message", result);
        }

        #[test]
        fn transform_response_with_tool_use() {
            let adapter = AnthropicAdapter::new();
            let response = json!({
                "id": "msg_02HGtRCzFPvKq8xKn4CTbN9M",
                "type": "message",
                "role": "assistant",
                "content": [
                    {
                        "type": "text",
                        "text": "I'll check the weather for you."
                    },
                    {
                        "type": "tool_use",
                        "id": "toolu_01A09q90qw90lq9179354",
                        "name": "get_weather",
                        "input": {"location": "San Francisco", "unit": "celsius"}
                    }
                ],
                "model": "claude-3-opus-20240229",
                "stop_reason": "tool_use",
                "stop_sequence": null,
                "usage": {
                    "input_tokens": 30,
                    "output_tokens": 45
                }
            });

            let result = adapter.transform_response(response).unwrap();
            insta::assert_json_snapshot!("anthropic_success_tool_use", result);
        }

        #[test]
        fn transform_response_multiple_text_blocks() {
            let adapter = AnthropicAdapter::new();
            let response = json!({
                "id": "msg_03multi",
                "type": "message",
                "role": "assistant",
                "content": [
                    {"type": "text", "text": "Here is step one. "},
                    {"type": "text", "text": "And here is step two."}
                ],
                "model": "claude-3-sonnet-20240229",
                "stop_reason": "end_turn",
                "usage": {
                    "input_tokens": 8,
                    "output_tokens": 12
                }
            });

            let result = adapter.transform_response(response).unwrap();
            // Multiple text blocks should be concatenated
            assert_eq!(result.content, "Here is step one. And here is step two.");
            assert_eq!(result.usage.total_tokens, 20);
        }
    }

    mod errors {
        use gateway::providers::anthropic::AnthropicAdapter;
        use gateway::providers::types::ProviderAdapter;
        use serde_json::json;

        #[test]
        fn transform_response_missing_content_returns_error() {
            let adapter = AnthropicAdapter::new();
            let response = json!({
                "id": "msg_err",
                "type": "error",
                "error": {
                    "type": "rate_limit_error",
                    "message": "Rate limit exceeded. Please retry after 1 second."
                }
            });
            let result = adapter.transform_response(response);
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(
                err.contains("No content"),
                "Expected 'No content' error, got: {err}"
            );
        }

        #[test]
        fn transform_response_null_content_returns_error() {
            let adapter = AnthropicAdapter::new();
            let response = json!({
                "id": "msg_err_null",
                "type": "message",
                "content": null,
                "usage": {}
            });
            let result = adapter.transform_response(response);
            assert!(result.is_err());
        }

        #[test]
        fn transform_response_empty_content_produces_empty_text() {
            let adapter = AnthropicAdapter::new();
            let response = json!({
                "id": "msg_empty",
                "type": "message",
                "content": [],
                "stop_reason": "end_turn",
                "usage": {"input_tokens": 5, "output_tokens": 0}
            });
            let result = adapter.transform_response(response).unwrap();
            assert_eq!(result.content, "");
        }

        #[test]
        fn transform_response_missing_usage_defaults_to_zero() {
            let adapter = AnthropicAdapter::new();
            let response = json!({
                "id": "msg_no_usage",
                "content": [{"type": "text", "text": "Hello"}],
                "stop_reason": "end_turn"
            });
            let result = adapter.transform_response(response).unwrap();
            assert_eq!(result.usage.prompt_tokens, 0);
            assert_eq!(result.usage.completion_tokens, 0);
            assert_eq!(result.usage.total_tokens, 0);
        }
    }

    mod request_transform {
        use gateway::providers::anthropic::AnthropicAdapter;
        use gateway::providers::types::{
            ContentPart, Message, MessageContent, ProviderAdapter, ProviderRequest, Tool,
            ToolChoice,
        };
        use rstest::rstest;
        use serde_json::json;

        #[test]
        fn simple_text_request() {
            let adapter = AnthropicAdapter::new();
            let request = ProviderRequest {
                messages: vec![Message {
                    role: "user".to_string(),
                    content: MessageContent::Text("Hello, Claude!".to_string()),
                    name: None,
                }],
                model: "claude-3-opus-20240229".to_string(),
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
            insta::assert_json_snapshot!("anthropic_request_simple_text", transformed);
        }

        #[test]
        fn request_with_system_prompt() {
            let adapter = AnthropicAdapter::new();
            let request = ProviderRequest {
                messages: vec![
                    Message {
                        role: "system".to_string(),
                        content: MessageContent::Text(
                            "You are a helpful coding assistant.".to_string(),
                        ),
                        name: None,
                    },
                    Message {
                        role: "user".to_string(),
                        content: MessageContent::Text("Write a Rust function".to_string()),
                        name: None,
                    },
                ],
                model: "claude-3-opus-20240229".to_string(),
                max_tokens: Some(2048),
                temperature: None,
                top_p: None,
                stop: None,
                stream: false,
                system: None,
                tools: None,
                tool_choice: None,
            };

            let transformed = adapter.transform_request(&request);
            insta::assert_json_snapshot!("anthropic_request_with_system", transformed);
        }

        #[test]
        fn request_with_tools() {
            let adapter = AnthropicAdapter::new();
            let request = ProviderRequest {
                messages: vec![Message {
                    role: "user".to_string(),
                    content: MessageContent::Text(
                        "Search for recent papers on LLM routing".to_string(),
                    ),
                    name: None,
                }],
                model: "claude-3-sonnet-20240229".to_string(),
                max_tokens: Some(4096),
                temperature: None,
                top_p: None,
                stop: None,
                stream: false,
                system: None,
                tools: Some(vec![
                    Tool::function("search_papers", "Search academic papers").with_parameters(
                        json!({
                            "type": "object",
                            "properties": {
                                "query": {"type": "string", "description": "Search query"},
                                "year": {"type": "integer", "description": "Publication year"}
                            },
                            "required": ["query"]
                        }),
                    ),
                ]),
                tool_choice: Some(ToolChoice::Auto),
            };

            let transformed = adapter.transform_request(&request);
            insta::assert_json_snapshot!("anthropic_request_with_tools", transformed);
        }

        #[test]
        fn request_with_image_url_parts() {
            let adapter = AnthropicAdapter::new();
            let request = ProviderRequest {
                messages: vec![Message {
                    role: "user".to_string(),
                    content: MessageContent::Parts(vec![
                        ContentPart::text("What is in this image?"),
                        ContentPart::image_url("https://example.com/photo.png"),
                    ]),
                    name: None,
                }],
                model: "claude-3-opus-20240229".to_string(),
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
            insta::assert_json_snapshot!("anthropic_request_image_url", transformed);
        }

        #[rstest]
        #[case::default(None, "https://api.anthropic.com/v1/messages")]
        #[case::custom(
            Some("https://proxy.example.com/v2"),
            "https://proxy.example.com/v2/messages"
        )]
        #[case::trailing_slash(
            Some("https://proxy.example.com/v2/"),
            "https://proxy.example.com/v2/messages"
        )]
        #[case::empty_string(Some(""), "https://api.anthropic.com/v1/messages")]
        #[case::localhost(Some("http://localhost:8080"), "http://localhost:8080/messages")]
        fn get_endpoint(#[case] base_url: Option<&str>, #[case] expected: &str) {
            let adapter = AnthropicAdapter::new();
            assert_eq!(adapter.get_endpoint(base_url, "claude-3-opus"), expected);
        }

        #[test]
        fn get_endpoint_model_id_ignored() {
            let adapter = AnthropicAdapter::new();
            let e1 = adapter.get_endpoint(None, "claude-3-opus");
            let e2 = adapter.get_endpoint(None, "claude-3-sonnet");
            assert_eq!(
                e1, e2,
                "Anthropic endpoint should not include model in path"
            );
        }

        #[test]
        fn build_headers_includes_anthropic_auth() {
            let adapter = AnthropicAdapter::new();
            let headers = adapter.build_headers("sk-ant-test-key");

            assert_eq!(headers.len(), 3);

            let api_key = headers.iter().find(|(k, _)| k == "x-api-key");
            assert!(api_key.is_some());
            assert_eq!(api_key.unwrap().1, "sk-ant-test-key");

            let version = headers.iter().find(|(k, _)| k == "anthropic-version");
            assert!(version.is_some());
            assert_eq!(version.unwrap().1, "2023-06-01");

            let content_type = headers.iter().find(|(k, _)| k == "content-type");
            assert!(content_type.is_some());
            assert_eq!(content_type.unwrap().1, "application/json");
        }
    }

    mod wiremock_round_trip {
        use gateway::providers::anthropic::AnthropicAdapter;
        use gateway::providers::types::{
            Message, MessageContent, ProviderAdapter, ProviderRequest,
        };
        use serde_json::json;

        #[tokio::test]
        #[ignore = "TODO: requires HTTP execution layer (see src/routes.rs:449)"]
        async fn anthropic_full_round_trip() {
            let mock_server = wiremock::MockServer::start().await;
            let adapter = AnthropicAdapter::with_base_url(mock_server.uri());

            wiremock::Mock::given(wiremock::matchers::method("POST"))
                .and(wiremock::matchers::path("/messages"))
                .and(wiremock::matchers::header("x-api-key", "sk-ant-test"))
                .and(wiremock::matchers::header(
                    "anthropic-version",
                    "2023-06-01",
                ))
                .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                    "id": "msg_wiremock",
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "text", "text": "Wiremock response"}],
                    "model": "claude-3-opus-20240229",
                    "stop_reason": "end_turn",
                    "usage": {"input_tokens": 5, "output_tokens": 3}
                })))
                .mount(&mock_server)
                .await;

            let request = ProviderRequest {
                messages: vec![Message {
                    role: "user".to_string(),
                    content: MessageContent::Text("Hello via wiremock".to_string()),
                    name: None,
                }],
                model: "claude-3-opus-20240229".to_string(),
                max_tokens: Some(256),
                temperature: None,
                top_p: None,
                stop: None,
                stream: false,
                system: None,
                tools: None,
                tool_choice: None,
            };

            let endpoint = adapter.get_endpoint(None, "claude-3-opus");
            let _body = adapter.transform_request(&request);
            let _headers = adapter.build_headers("sk-ant-test");
            let _ = endpoint;
            // TODO: send request and verify response once HTTP layer is ready
        }

        #[tokio::test]
        #[ignore = "TODO: requires HTTP execution layer"]
        async fn anthropic_rate_limit_response() {
            let mock_server = wiremock::MockServer::start().await;
            let _adapter = AnthropicAdapter::with_base_url(mock_server.uri());

            wiremock::Mock::given(wiremock::matchers::method("POST"))
                .respond_with(wiremock::ResponseTemplate::new(429).set_body_json(json!({
                    "type": "error",
                    "error": {
                        "type": "rate_limit_error",
                        "message": "Number of request tokens has exceeded your rate limit."
                    }
                })))
                .mount(&mock_server)
                .await;
            // TODO: verify 429 handling when HTTP layer is ready
        }

        #[tokio::test]
        #[ignore = "TODO: requires HTTP execution layer"]
        async fn anthropic_auth_error_response() {
            let mock_server = wiremock::MockServer::start().await;
            let _adapter = AnthropicAdapter::with_base_url(mock_server.uri());

            wiremock::Mock::given(wiremock::matchers::method("POST"))
                .respond_with(wiremock::ResponseTemplate::new(401).set_body_json(json!({
                    "type": "error",
                    "error": {
                        "type": "authentication_error",
                        "message": "invalid x-api-key"
                    }
                })))
                .mount(&mock_server)
                .await;
            // TODO: verify 401 handling when HTTP layer is ready
        }

        #[tokio::test]
        #[ignore = "TODO: requires HTTP execution layer"]
        async fn anthropic_server_error_response() {
            let mock_server = wiremock::MockServer::start().await;
            let _adapter = AnthropicAdapter::with_base_url(mock_server.uri());

            wiremock::Mock::given(wiremock::matchers::method("POST"))
                .respond_with(wiremock::ResponseTemplate::new(500).set_body_json(json!({
                    "type": "error",
                    "error": {
                        "type": "api_error",
                        "message": "Internal server error"
                    }
                })))
                .mount(&mock_server)
                .await;
            // TODO: verify 500 handling when HTTP layer is ready
        }
    }
}

#[cfg(test)]
mod google {
    mod success {
        use gateway::providers::google::GoogleAdapter;
        use gateway::providers::types::ProviderAdapter;
        use serde_json::json;

        #[test]
        fn transform_response_generate_content() {
            let adapter = GoogleAdapter::new();
            let response = json!({
                "candidates": [{
                    "content": {
                        "parts": [{"text": "Hello! I'm Gemini, how can I help?"}],
                        "role": "model"
                    },
                    "finishReason": "STOP",
                    "index": 0
                }],
                "usageMetadata": {
                    "promptTokenCount": 10,
                    "candidatesTokenCount": 8,
                    "totalTokenCount": 18
                },
                "modelVersion": "gemini-1.5-pro-002"
            });

            let result = adapter.transform_response(response).unwrap();
            // Google adapter generates UUID-based IDs, so redact them for snapshot stability
            insta::assert_json_snapshot!("google_success_generate_content", result, {
                ".id" => insta::dynamic_redaction(|value, _path| {
                    let id = value.as_str().unwrap_or("");
                    assert!(
                        id.starts_with("gemini-"),
                        "Expected gemini-prefixed ID, got: {id}"
                    );
                    "[uuid-redacted]"
                }),
            });
        }

        #[test]
        fn transform_response_with_function_call() {
            let adapter = GoogleAdapter::new();
            let response = json!({
                "candidates": [{
                    "content": {
                        "parts": [
                            {"text": "Let me look that up."},
                            {"functionCall": {"name": "get_weather", "args": {"location": "Tokyo", "unit": "celsius"}}}
                        ],
                        "role": "model"
                    },
                    "finishReason": "STOP"
                }],
                "usageMetadata": {
                    "promptTokenCount": 25,
                    "candidatesTokenCount": 18,
                    "totalTokenCount": 43
                }
            });

            let result = adapter.transform_response(response).unwrap();
            // Redact all UUID-generated fields
            insta::assert_json_snapshot!("google_success_function_call", result, {
                ".id" => insta::dynamic_redaction(|value, _path| {
                    let id = value.as_str().unwrap_or("");
                    assert!(id.starts_with("gemini-"), "Expected gemini prefix, got: {id}");
                    "[uuid-redacted]"
                }),
                ".tool_calls[].id" => insta::dynamic_redaction(|value, _path| {
                    let id = value.as_str().unwrap_or("");
                    assert!(id.starts_with("call_"), "Expected call_ prefix, got: {id}");
                    "[uuid-redacted]"
                }),
            });
        }

        #[test]
        fn transform_response_multiple_text_parts() {
            let adapter = GoogleAdapter::new();
            let response = json!({
                "candidates": [{
                    "content": {
                        "parts": [
                            {"text": "Step 1: "},
                            {"text": "Do this. "},
                            {"text": "Step 2: Do that."}
                        ],
                        "role": "model"
                    },
                    "finishReason": "STOP"
                }],
                "usageMetadata": {
                    "promptTokenCount": 5,
                    "candidatesTokenCount": 10,
                    "totalTokenCount": 15
                }
            });

            let result = adapter.transform_response(response).unwrap();
            // All text parts should be concatenated
            assert_eq!(result.content, "Step 1: Do this. Step 2: Do that.");
            assert_eq!(result.usage.total_tokens, 15);
        }
    }

    mod errors {
        use gateway::providers::google::GoogleAdapter;
        use gateway::providers::types::ProviderAdapter;
        use serde_json::json;

        #[test]
        fn transform_response_missing_candidates_returns_error() {
            let adapter = GoogleAdapter::new();
            let response = json!({
                "error": {
                    "code": 429,
                    "message": "Resource exhausted",
                    "status": "RESOURCE_EXHAUSTED"
                }
            });
            let result = adapter.transform_response(response);
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(
                err.contains("No candidates"),
                "Expected 'No candidates' error, got: {err}"
            );
        }

        #[test]
        fn transform_response_empty_candidates_returns_error() {
            let adapter = GoogleAdapter::new();
            let response = json!({"candidates": []});
            let result = adapter.transform_response(response);
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(
                err.contains("Empty candidates"),
                "Expected 'Empty candidates' error, got: {err}"
            );
        }

        #[test]
        fn transform_response_null_body_returns_error() {
            let adapter = GoogleAdapter::new();
            let response = json!(null);
            let result = adapter.transform_response(response);
            assert!(result.is_err());
        }

        #[test]
        fn transform_response_missing_usage_defaults_to_zero() {
            let adapter = GoogleAdapter::new();
            let response = json!({
                "candidates": [{
                    "content": {
                        "parts": [{"text": "Hello"}],
                        "role": "model"
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
        fn transform_response_missing_finish_reason_defaults_to_unknown() {
            let adapter = GoogleAdapter::new();
            let response = json!({
                "candidates": [{
                    "content": {
                        "parts": [{"text": "Hello"}],
                        "role": "model"
                    }
                }],
                "usageMetadata": {}
            });
            let result = adapter.transform_response(response).unwrap();
            assert_eq!(result.finish_reason, "UNKNOWN");
        }
    }

    mod request_transform {
        use gateway::providers::google::GoogleAdapter;
        use gateway::providers::types::{
            Message, MessageContent, ProviderAdapter, ProviderRequest, Tool, ToolChoice,
        };
        use rstest::rstest;
        use serde_json::json;

        #[test]
        fn simple_text_request() {
            let adapter = GoogleAdapter::new();
            let request = ProviderRequest {
                messages: vec![Message {
                    role: "user".to_string(),
                    content: MessageContent::Text("Hello, Gemini!".to_string()),
                    name: None,
                }],
                model: "gemini-1.5-pro".to_string(),
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
            insta::assert_json_snapshot!("google_request_simple_text", transformed);
        }

        #[test]
        fn request_with_system_instruction() {
            let adapter = GoogleAdapter::new();
            let request = ProviderRequest {
                messages: vec![
                    Message {
                        role: "system".to_string(),
                        content: MessageContent::Text("You are a helpful assistant.".to_string()),
                        name: None,
                    },
                    Message {
                        role: "user".to_string(),
                        content: MessageContent::Text("Explain Rust ownership.".to_string()),
                        name: None,
                    },
                ],
                model: "gemini-1.5-pro".to_string(),
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
            insta::assert_json_snapshot!("google_request_system_instruction", transformed);
        }

        #[test]
        fn request_with_tools() {
            let adapter = GoogleAdapter::new();
            let request = ProviderRequest {
                messages: vec![Message {
                    role: "user".to_string(),
                    content: MessageContent::Text("What is the weather?".to_string()),
                    name: None,
                }],
                model: "gemini-1.5-pro".to_string(),
                max_tokens: Some(2048),
                temperature: None,
                top_p: None,
                stop: None,
                stream: false,
                system: None,
                tools: Some(vec![
                    Tool::function("get_weather", "Get current weather").with_parameters(json!({
                        "type": "object",
                        "properties": {
                            "location": {"type": "string"},
                            "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
                        },
                        "required": ["location"]
                    })),
                ]),
                tool_choice: Some(ToolChoice::Auto),
            };

            let transformed = adapter.transform_request(&request);
            insta::assert_json_snapshot!("google_request_with_tools", transformed);
        }

        #[test]
        fn request_with_all_generation_config() {
            let adapter = GoogleAdapter::new();
            let request = ProviderRequest {
                messages: vec![Message {
                    role: "user".to_string(),
                    content: MessageContent::Text("Generate a poem".to_string()),
                    name: None,
                }],
                model: "gemini-1.5-pro".to_string(),
                max_tokens: Some(512),
                temperature: Some(0.9),
                top_p: Some(0.95),
                stop: Some(vec!["END".to_string(), "---".to_string()]),
                stream: false,
                system: Some("You are a poet.".to_string()),
                tools: None,
                tool_choice: None,
            };

            let transformed = adapter.transform_request(&request);
            insta::assert_json_snapshot!("google_request_all_config", transformed);
        }

        #[test]
        fn request_with_assistant_role_mapped_to_model() {
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
                    Message {
                        role: "user".to_string(),
                        content: MessageContent::Text("How are you?".to_string()),
                        name: None,
                    },
                ],
                model: "gemini-1.5-pro".to_string(),
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
            let contents = transformed["contents"]
                .as_array()
                .expect("contents should be array");
            assert_eq!(contents[0]["role"], "user");
            assert_eq!(
                contents[1]["role"], "model",
                "assistant should map to model"
            );
            assert_eq!(contents[2]["role"], "user");
        }

        #[test]
        fn request_with_tool_choice_required_maps_to_any() {
            let adapter = GoogleAdapter::new();
            let request = ProviderRequest {
                messages: vec![Message {
                    role: "user".to_string(),
                    content: MessageContent::Text("Search".to_string()),
                    name: None,
                }],
                model: "gemini-1.5-pro".to_string(),
                max_tokens: None,
                temperature: None,
                top_p: None,
                stop: None,
                stream: false,
                system: None,
                tools: Some(vec![Tool::function("search", "Search the web")]),
                tool_choice: Some(ToolChoice::Required),
            };

            let transformed = adapter.transform_request(&request);
            let mode = &transformed["tool_config"]["function_calling_config"]["mode"];
            assert_eq!(mode, "ANY", "Required should map to ANY in Gemini");
        }

        #[test]
        fn request_with_tool_choice_function_maps_to_any_with_name() {
            let adapter = GoogleAdapter::new();
            let request = ProviderRequest {
                messages: vec![Message {
                    role: "user".to_string(),
                    content: MessageContent::Text("Search".to_string()),
                    name: None,
                }],
                model: "gemini-1.5-pro".to_string(),
                max_tokens: None,
                temperature: None,
                top_p: None,
                stop: None,
                stream: false,
                system: None,
                tools: Some(vec![Tool::function("search", "Search the web")]),
                tool_choice: Some(ToolChoice::Function {
                    name: "search".to_string(),
                }),
            };

            let transformed = adapter.transform_request(&request);
            let config = &transformed["tool_config"]["function_calling_config"];
            assert_eq!(config["mode"], "ANY");
            assert_eq!(config["allowed_function_names"][0], "search");
        }

        #[rstest]
        #[case::default(
            None,
            "gemini-1.5-pro",
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-pro:generateContent"
        )]
        #[case::custom(
            Some("https://proxy.example.com"),
            "gemini-pro",
            "https://proxy.example.com/models/gemini-pro:generateContent"
        )]
        #[case::trailing_slash(
            Some("https://proxy.example.com/"),
            "gemini-pro",
            "https://proxy.example.com/models/gemini-pro:generateContent"
        )]
        #[case::empty_string(
            Some(""),
            "gemini-pro",
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent"
        )]
        fn get_endpoint(
            #[case] base_url: Option<&str>,
            #[case] model_id: &str,
            #[case] expected: &str,
        ) {
            let adapter = GoogleAdapter::new();
            assert_eq!(adapter.get_endpoint(base_url, model_id), expected);
        }

        #[test]
        fn get_endpoint_sanitizes_path_traversal() {
            let adapter = GoogleAdapter::new();
            let endpoint = adapter.get_endpoint(None, "../../etc/passwd");
            // Path separators should be stripped
            assert_eq!(
                endpoint,
                "https://generativelanguage.googleapis.com/v1beta/models/....etcpasswd:generateContent"
            );
        }

        #[test]
        fn build_headers_includes_google_api_key() {
            let adapter = GoogleAdapter::new();
            let headers = adapter.build_headers("AIzaSyTestKey123");

            assert_eq!(headers.len(), 2);

            let api_key = headers.iter().find(|(k, _)| k == "x-goog-api-key");
            assert!(api_key.is_some());
            assert_eq!(api_key.unwrap().1, "AIzaSyTestKey123");

            let content_type = headers.iter().find(|(k, _)| k == "Content-Type");
            assert!(content_type.is_some());
            assert_eq!(content_type.unwrap().1, "application/json");
        }
    }

    mod wiremock_round_trip {
        use gateway::providers::google::GoogleAdapter;
        use gateway::providers::types::{
            Message, MessageContent, ProviderAdapter, ProviderRequest,
        };
        use serde_json::json;

        #[tokio::test]
        #[ignore = "TODO: requires HTTP execution layer (see src/routes.rs:449)"]
        async fn google_full_round_trip() {
            let mock_server = wiremock::MockServer::start().await;
            let adapter = GoogleAdapter::with_base_url(mock_server.uri());

            wiremock::Mock::given(wiremock::matchers::method("POST"))
                .and(wiremock::matchers::path(
                    "/models/gemini-1.5-pro:generateContent",
                ))
                .and(wiremock::matchers::header("x-goog-api-key", "test-key"))
                .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                    "candidates": [{
                        "content": {
                            "parts": [{"text": "Wiremock response"}],
                            "role": "model"
                        },
                        "finishReason": "STOP"
                    }],
                    "usageMetadata": {
                        "promptTokenCount": 5,
                        "candidatesTokenCount": 3,
                        "totalTokenCount": 8
                    }
                })))
                .mount(&mock_server)
                .await;

            let request = ProviderRequest {
                messages: vec![Message {
                    role: "user".to_string(),
                    content: MessageContent::Text("Hello via wiremock".to_string()),
                    name: None,
                }],
                model: "gemini-1.5-pro".to_string(),
                max_tokens: Some(256),
                temperature: None,
                top_p: None,
                stop: None,
                stream: false,
                system: None,
                tools: None,
                tool_choice: None,
            };

            let endpoint = adapter.get_endpoint(None, "gemini-1.5-pro");
            let _body = adapter.transform_request(&request);
            let _headers = adapter.build_headers("test-key");
            let _ = endpoint;
            // TODO: send request and verify response once HTTP layer is ready
        }

        #[tokio::test]
        #[ignore = "TODO: requires HTTP execution layer"]
        async fn google_rate_limit_response() {
            let mock_server = wiremock::MockServer::start().await;
            let _adapter = GoogleAdapter::with_base_url(mock_server.uri());

            wiremock::Mock::given(wiremock::matchers::method("POST"))
                .respond_with(wiremock::ResponseTemplate::new(429).set_body_json(json!({
                    "error": {
                        "code": 429,
                        "message": "Resource has been exhausted",
                        "status": "RESOURCE_EXHAUSTED"
                    }
                })))
                .mount(&mock_server)
                .await;
            // TODO: verify 429 handling when HTTP layer is ready
        }

        #[tokio::test]
        #[ignore = "TODO: requires HTTP execution layer"]
        async fn google_auth_error_response() {
            let mock_server = wiremock::MockServer::start().await;
            let _adapter = GoogleAdapter::with_base_url(mock_server.uri());

            wiremock::Mock::given(wiremock::matchers::method("POST"))
                .respond_with(wiremock::ResponseTemplate::new(401).set_body_json(json!({
                    "error": {
                        "code": 401,
                        "message": "API key not valid",
                        "status": "UNAUTHENTICATED"
                    }
                })))
                .mount(&mock_server)
                .await;
            // TODO: verify 401 handling when HTTP layer is ready
        }

        #[tokio::test]
        #[ignore = "TODO: requires HTTP execution layer"]
        async fn google_server_error_response() {
            let mock_server = wiremock::MockServer::start().await;
            let _adapter = GoogleAdapter::with_base_url(mock_server.uri());

            wiremock::Mock::given(wiremock::matchers::method("POST"))
                .respond_with(wiremock::ResponseTemplate::new(500).set_body_json(json!({
                    "error": {
                        "code": 500,
                        "message": "Internal error",
                        "status": "INTERNAL"
                    }
                })))
                .mount(&mock_server)
                .await;
            // TODO: verify 500 handling when HTTP layer is ready
        }
    }
}

#[cfg(test)]
mod cross_provider {
    //! Cross-provider comparison tests validating consistent behavior.

    use gateway::providers::types::ProviderAdapter;
    use rstest::rstest;

    fn all_adapters() -> Vec<Box<dyn ProviderAdapter>> {
        vec![
            Box::new(gateway::providers::openai::OpenAIAdapter::new()),
            Box::new(gateway::providers::anthropic::AnthropicAdapter::new()),
            Box::new(gateway::providers::google::GoogleAdapter::new()),
        ]
    }

    #[rstest]
    #[case::openai("openai")]
    #[case::anthropic("anthropic")]
    #[case::google("google")]
    fn provider_names_are_unique(#[case] expected_name: &str) {
        let adapters = all_adapters();
        let names: Vec<&str> = adapters.iter().map(|a| a.provider_name()).collect();
        let count = names.iter().filter(|&&n| n == expected_name).count();
        assert_eq!(
            count, 1,
            "Provider name '{expected_name}' should appear exactly once"
        );
    }

    #[test]
    fn all_adapters_have_non_empty_default_endpoints() {
        for adapter in all_adapters() {
            let endpoint = adapter.get_endpoint(None, "test-model");
            assert!(
                !endpoint.is_empty(),
                "Adapter '{}' returned empty default endpoint",
                adapter.provider_name()
            );
            assert!(
                endpoint.starts_with("https://"),
                "Adapter '{}' default endpoint should use HTTPS: {endpoint}",
                adapter.provider_name()
            );
        }
    }

    #[test]
    fn all_adapters_include_content_type_header() {
        for adapter in all_adapters() {
            let headers = adapter.build_headers("test-key");
            let has_content_type = headers
                .iter()
                .any(|(k, v)| k.eq_ignore_ascii_case("content-type") && v == "application/json");
            assert!(
                has_content_type,
                "Adapter '{}' should include Content-Type: application/json",
                adapter.provider_name()
            );
        }
    }

    #[test]
    fn all_adapters_include_api_key_in_headers() {
        for adapter in all_adapters() {
            let headers = adapter.build_headers("my-secret-key");
            let has_key = headers.iter().any(|(_, v)| v.contains("my-secret-key"));
            assert!(
                has_key,
                "Adapter '{}' should include API key in headers",
                adapter.provider_name()
            );
        }
    }
}
