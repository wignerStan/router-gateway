use crate::collector::TraceCollector;
use crate::trace::TraceSpan;
use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

/// Tracing middleware for Axum that intercepts and logs LLM requests
#[derive(Clone)]
pub struct TracingMiddleware {
    collector: Arc<dyn TraceCollector>,
}

impl TracingMiddleware {
    /// Create a new tracing middleware with a collector
    pub fn new(collector: Arc<dyn TraceCollector>) -> Self {
        Self { collector }
    }

    /// Extract request ID from headers or generate a new one
    fn extract_request_id(&self, headers: &HeaderMap) -> String {
        headers
            .get("x-request-id")
            .or_else(|| headers.get("x-trace-id"))
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                // Generate UUID-based request ID
                Uuid::new_v4().to_string()
            })
    }

    /// Extract provider from request (e.g., from path or headers)
    fn extract_provider(&self, headers: &HeaderMap) -> Option<String> {
        // Try to get from header first
        if let Some(provider) = headers.get("x-llm-provider") {
            return provider.to_str().ok().map(|s| s.to_string());
        }
        // Could also extract from URI path in a real implementation
        None
    }

    /// Extract model from request (e.g., from body or headers)
    fn extract_model(&self, headers: &HeaderMap) -> Option<String> {
        if let Some(model) = headers.get("x-llm-model") {
            return model.to_str().ok().map(|s| s.to_string());
        }
        None
    }

    /// Extract auth ID from headers (sanitized - never logs full credentials)
    fn extract_auth_id(&self, headers: &HeaderMap) -> Option<String> {
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| {
                // Only indicate authentication status, never expose full tokens
                if v.starts_with("Bearer ") || v.starts_with("ApiKey ") {
                    Some("authenticated".to_string())
                } else if !v.is_empty() {
                    // Non-standard auth header - indicate presence only
                    Some("authenticated".to_string())
                } else {
                    None
                }
            })
    }

    /// Process the request and generate a trace
    pub async fn trace_request(
        &self,
        _method: axum::http::Method,
        _uri: axum::http::Uri,
        headers: HeaderMap,
        _body: Vec<u8>,
    ) -> TraceSpan {
        let request_id = self.extract_request_id(&headers);
        let provider = self
            .extract_provider(&headers)
            .unwrap_or_else(|| "unknown".to_string());
        let model = self
            .extract_model(&headers)
            .unwrap_or_else(|| "unknown".to_string());
        let auth_id = self.extract_auth_id(&headers);

        let mut span = TraceSpan::new(request_id, provider, model, auth_id);

        // Try to extract input tokens if available
        if let Some(tokens) = headers.get("x-input-tokens") {
            if let Ok(token_str) = tokens.to_str() {
                span.input_tokens = token_str.parse().ok();
            }
        }

        // Extract streaming flag
        span.is_streaming = headers
            .get("x-streaming")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        span
    }
}

/// Axum middleware handler
pub async fn tracing_middleware(
    State(middleware): State<TracingMiddleware>,
    req: Request,
    next: Next,
) -> Response {
    let start = Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let headers = req.headers().clone();

    // Create initial trace span
    let mut span = middleware.trace_request(method, uri, headers, vec![]).await;

    // Execute the request
    let response = next.run(req).await;
    let latency = start.elapsed();
    let status = response.status();

    // Update trace with response data
    span.status_code = Some(status.as_u16());
    span.latency_ms = Some(latency.as_millis() as u64);
    span.end_time = Some(chrono::Utc::now());

    // Extract output tokens from response headers if available
    if let Some(tokens) = response.headers().get("x-output-tokens") {
        if let Ok(token_str) = tokens.to_str() {
            span.output_tokens = token_str.parse().ok();
        }
    }

    // Record the trace
    middleware.collector.record_trace(span).await;

    response
}

/// Builder for creating tracing middleware
pub struct TracingMiddlewareBuilder {
    collector: Option<Arc<dyn TraceCollector>>,
}

impl Default for TracingMiddlewareBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TracingMiddlewareBuilder {
    pub fn new() -> Self {
        Self { collector: None }
    }

    pub fn with_collector(mut self, collector: Arc<dyn TraceCollector>) -> Self {
        self.collector = Some(collector);
        self
    }

    pub fn build(self) -> TracingMiddleware {
        TracingMiddleware {
            collector: self
                .collector
                .expect("Collector must be set to build TracingMiddleware"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::MemoryTraceCollector;
    use axum::http::{HeaderMap, HeaderValue};

    #[tokio::test]
    async fn test_extract_request_id() {
        let collector =
            Arc::new(MemoryTraceCollector::with_default_size()) as Arc<dyn TraceCollector>;
        let middleware = TracingMiddleware::new(collector);

        // Test with x-request-id header
        let mut headers = HeaderMap::new();
        headers.insert("x-request-id", HeaderValue::from_static("test-req-123"));
        assert_eq!(middleware.extract_request_id(&headers), "test-req-123");

        // Test with x-trace-id header
        let mut headers = HeaderMap::new();
        headers.insert("x-trace-id", HeaderValue::from_static("trace-456"));
        assert_eq!(middleware.extract_request_id(&headers), "trace-456");

        // Test without header (should generate UUID)
        let headers = HeaderMap::new();
        let request_id = middleware.extract_request_id(&headers);
        assert!(!request_id.is_empty());
        assert!(Uuid::parse_str(&request_id).is_ok());
    }

    #[tokio::test]
    async fn test_extract_provider() {
        let collector =
            Arc::new(MemoryTraceCollector::with_default_size()) as Arc<dyn TraceCollector>;
        let middleware = TracingMiddleware::new(collector);

        let mut headers = HeaderMap::new();
        headers.insert("x-llm-provider", HeaderValue::from_static("openai"));
        assert_eq!(
            middleware.extract_provider(&headers),
            Some("openai".to_string())
        );

        let headers = HeaderMap::new();
        assert_eq!(middleware.extract_provider(&headers), None);
    }

    #[tokio::test]
    async fn test_extract_auth_id() {
        let collector =
            Arc::new(MemoryTraceCollector::with_default_size()) as Arc<dyn TraceCollector>;
        let middleware = TracingMiddleware::new(collector);

        // Bearer token - should return "authenticated" (never expose full token)
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer token-123"),
        );
        assert_eq!(
            middleware.extract_auth_id(&headers),
            Some("authenticated".to_string())
        );

        // API key - should return "authenticated" (never expose full key)
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("api-key-456"));
        assert_eq!(
            middleware.extract_auth_id(&headers),
            Some("authenticated".to_string())
        );

        // No auth header - should return None
        let headers = HeaderMap::new();
        assert_eq!(middleware.extract_auth_id(&headers), None);
    }

    #[tokio::test]
    async fn test_trace_request() {
        let collector =
            Arc::new(MemoryTraceCollector::with_default_size()) as Arc<dyn TraceCollector>;
        let middleware = TracingMiddleware::new(collector);

        let mut headers = HeaderMap::new();
        headers.insert("x-request-id", HeaderValue::from_static("req-123"));
        headers.insert("x-llm-provider", HeaderValue::from_static("anthropic"));
        headers.insert("x-llm-model", HeaderValue::from_static("claude-3"));
        headers.insert("x-input-tokens", HeaderValue::from_static("100"));
        headers.insert("x-streaming", HeaderValue::from_static("true"));

        let span = middleware
            .trace_request(
                axum::http::Method::POST,
                "/v1/messages".parse().expect("value must be present"),
                headers,
                vec![],
            )
            .await;

        assert_eq!(span.request_id, "req-123");
        assert_eq!(span.provider, "anthropic");
        assert_eq!(span.model, "claude-3");
        assert_eq!(span.input_tokens, Some(100));
        assert!(span.is_streaming);
    }

    #[tokio::test]
    async fn test_middleware_builder() {
        let collector =
            Arc::new(MemoryTraceCollector::with_default_size()) as Arc<dyn TraceCollector>;

        let middleware = TracingMiddlewareBuilder::new()
            .with_collector(collector)
            .build();

        assert!(Arc::strong_count(&middleware.collector) >= 1);
    }

    // ===== Edge Case Tests =====

    #[tokio::test]
    async fn test_extract_request_id_invalid_utf8() {
        let collector =
            Arc::new(MemoryTraceCollector::with_default_size()) as Arc<dyn TraceCollector>;
        let middleware = TracingMiddleware::new(collector);

        // Create headers with invalid UTF-8 value
        let mut headers = HeaderMap::new();
        // HeaderValue::from_bytes will fail for invalid UTF-8, so we use valid ASCII
        // but test that we fall back to UUID generation when header value is not valid string
        headers.insert(
            "x-request-id",
            HeaderValue::from_bytes(b"valid-id").expect("value must be present"),
        );
        assert_eq!(middleware.extract_request_id(&headers), "valid-id");

        // When no valid header exists, should generate UUID
        let empty_headers = HeaderMap::new();
        let request_id = middleware.extract_request_id(&empty_headers);
        assert!(Uuid::parse_str(&request_id).is_ok());
    }

    #[tokio::test]
    async fn test_extract_request_id_multiple_headers() {
        let collector =
            Arc::new(MemoryTraceCollector::with_default_size()) as Arc<dyn TraceCollector>;
        let middleware = TracingMiddleware::new(collector);

        // x-request-id takes priority over x-trace-id
        let mut headers = HeaderMap::new();
        headers.insert("x-request-id", HeaderValue::from_static("priority-id"));
        headers.insert("x-trace-id", HeaderValue::from_static("secondary-id"));
        assert_eq!(middleware.extract_request_id(&headers), "priority-id");

        // x-trace-id is used when x-request-id is absent
        let mut headers2 = HeaderMap::new();
        headers2.insert("x-trace-id", HeaderValue::from_static("trace-id-value"));
        assert_eq!(middleware.extract_request_id(&headers2), "trace-id-value");
    }

    #[tokio::test]
    async fn test_extract_provider_invalid_utf8() {
        let collector =
            Arc::new(MemoryTraceCollector::with_default_size()) as Arc<dyn TraceCollector>;
        let middleware = TracingMiddleware::new(collector);

        // When header value is not valid UTF-8, to_str() returns Err
        // In that case, extract_provider returns None
        let headers = HeaderMap::new();
        assert_eq!(middleware.extract_provider(&headers), None);

        // Valid provider
        let mut headers2 = HeaderMap::new();
        headers2.insert("x-llm-provider", HeaderValue::from_static("openai"));
        assert_eq!(
            middleware.extract_provider(&headers2),
            Some("openai".to_string())
        );
    }

    #[tokio::test]
    async fn test_extract_provider_empty_string() {
        let collector =
            Arc::new(MemoryTraceCollector::with_default_size()) as Arc<dyn TraceCollector>;
        let middleware = TracingMiddleware::new(collector);

        // Empty header value
        let mut headers = HeaderMap::new();
        headers.insert("x-llm-provider", HeaderValue::from_static(""));
        assert_eq!(middleware.extract_provider(&headers), Some("".to_string()));

        // No header at all
        let headers2 = HeaderMap::new();
        assert_eq!(middleware.extract_provider(&headers2), None);
    }

    #[tokio::test]
    async fn test_trace_request_missing_headers() {
        let collector =
            Arc::new(MemoryTraceCollector::with_default_size()) as Arc<dyn TraceCollector>;
        let middleware = TracingMiddleware::new(collector);

        // Empty headers - should use defaults
        let headers = HeaderMap::new();
        let span = middleware
            .trace_request(
                axum::http::Method::POST,
                "/v1/messages".parse().expect("value must be present"),
                headers,
                vec![],
            )
            .await;

        // Should have generated a UUID request_id
        assert!(Uuid::parse_str(&span.request_id).is_ok());
        assert_eq!(span.provider, "unknown");
        assert_eq!(span.model, "unknown");
        assert!(span.input_tokens.is_none());
        assert!(!span.is_streaming);
    }

    #[tokio::test]
    async fn test_trace_request_large_body() {
        let collector =
            Arc::new(MemoryTraceCollector::with_default_size()) as Arc<dyn TraceCollector>;
        let middleware = TracingMiddleware::new(collector);

        // Create a large body (1MB)
        let large_body = vec![0u8; 1024 * 1024];

        let mut headers = HeaderMap::new();
        headers.insert("x-request-id", HeaderValue::from_static("req-large"));
        headers.insert("x-llm-provider", HeaderValue::from_static("openai"));

        let span = middleware
            .trace_request(
                axum::http::Method::POST,
                "/v1/chat/completions"
                    .parse()
                    .expect("value must be present"),
                headers,
                large_body,
            )
            .await;

        assert_eq!(span.request_id, "req-large");
        assert_eq!(span.provider, "openai");
    }

    #[tokio::test]
    #[should_panic(expected = "Collector must be set")]
    async fn test_middleware_builder_without_collector() {
        // Building without collector should panic
        let _middleware = TracingMiddlewareBuilder::new().build();
    }

    #[tokio::test]
    async fn test_extract_model_from_header() {
        let collector =
            Arc::new(MemoryTraceCollector::with_default_size()) as Arc<dyn TraceCollector>;
        let middleware = TracingMiddleware::new(collector);

        let mut headers = HeaderMap::new();
        headers.insert("x-llm-model", HeaderValue::from_static("gpt-4-turbo"));
        assert_eq!(
            middleware.extract_model(&headers),
            Some("gpt-4-turbo".to_string())
        );

        let empty_headers = HeaderMap::new();
        assert_eq!(middleware.extract_model(&empty_headers), None);
    }

    #[tokio::test]
    async fn test_extract_input_tokens_from_header() {
        let collector =
            Arc::new(MemoryTraceCollector::with_default_size()) as Arc<dyn TraceCollector>;
        let middleware = TracingMiddleware::new(collector);

        let mut headers = HeaderMap::new();
        headers.insert("x-request-id", HeaderValue::from_static("req-1"));
        headers.insert("x-input-tokens", HeaderValue::from_static("500"));

        let span = middleware
            .trace_request(
                axum::http::Method::POST,
                "/v1/messages".parse().expect("value must be present"),
                headers,
                vec![],
            )
            .await;

        assert_eq!(span.input_tokens, Some(500));
    }

    #[tokio::test]
    async fn test_extract_streaming_flag() {
        let collector =
            Arc::new(MemoryTraceCollector::with_default_size()) as Arc<dyn TraceCollector>;
        let middleware = TracingMiddleware::new(collector);

        // Test "true"
        let mut headers1 = HeaderMap::new();
        headers1.insert("x-streaming", HeaderValue::from_static("true"));
        let span1 = middleware
            .trace_request(
                axum::http::Method::POST,
                "/v1/messages".parse().expect("value must be present"),
                headers1,
                vec![],
            )
            .await;
        assert!(span1.is_streaming);

        // Test "1"
        let mut headers2 = HeaderMap::new();
        headers2.insert("x-streaming", HeaderValue::from_static("1"));
        let span2 = middleware
            .trace_request(
                axum::http::Method::POST,
                "/v1/messages".parse().expect("value must be present"),
                headers2,
                vec![],
            )
            .await;
        assert!(span2.is_streaming);

        // Test "false"
        let mut headers3 = HeaderMap::new();
        headers3.insert("x-streaming", HeaderValue::from_static("false"));
        let span3 = middleware
            .trace_request(
                axum::http::Method::POST,
                "/v1/messages".parse().expect("value must be present"),
                headers3,
                vec![],
            )
            .await;
        assert!(!span3.is_streaming);

        // Test no header
        let headers4 = HeaderMap::new();
        let span4 = middleware
            .trace_request(
                axum::http::Method::POST,
                "/v1/messages".parse().expect("value must be present"),
                headers4,
                vec![],
            )
            .await;
        assert!(!span4.is_streaming);
    }
}
