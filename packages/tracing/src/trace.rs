use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a single LLM request/response trace
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TraceSpan {
    /// Unique identifier for this trace
    pub trace_id: Uuid,
    /// Request identifier (may come from headers)
    pub request_id: String,
    /// LLM provider (e.g., "openai", "anthropic", "google")
    pub provider: String,
    /// Model name (e.g., "gpt-4", "claude-3-opus")
    pub model: String,
    /// Authentication/organization identifier
    pub auth_id: Option<String>,

    /// Timestamps
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,

    /// Request data
    pub input_tokens: Option<u32>,
    pub prompt: Option<String>,

    /// Response data
    pub output_tokens: Option<u32>,
    pub status_code: Option<u16>,
    pub latency_ms: Option<u64>,

    /// Metadata
    pub error_message: Option<String>,
    pub is_streaming: bool,
}

impl TraceSpan {
    /// Create a new trace span.
    ///
    /// Sets `start_time` to now. All optional fields (`end_time`, tokens,
    /// etc.) start as `None` until explicitly set.
    ///
    /// # Examples
    ///
    /// ```
    /// use llm_tracing::TraceSpan;
    ///
    /// let span = TraceSpan::new(
    ///     "req-123".to_string(),
    ///     "openai".to_string(),
    ///     "gpt-4".to_string(),
    ///     Some("user-456".to_string()),
    /// );
    ///
    /// assert_eq!(span.request_id, "req-123");
    /// assert_eq!(span.provider, "openai");
    /// assert!(span.end_time.is_none());
    /// ```
    pub fn new(
        request_id: String,
        provider: String,
        model: String,
        auth_id: Option<String>,
    ) -> Self {
        Self {
            trace_id: Uuid::new_v4(),
            request_id,
            provider,
            model,
            auth_id,
            start_time: Utc::now(),
            end_time: None,
            input_tokens: None,
            prompt: None,
            output_tokens: None,
            status_code: None,
            latency_ms: None,
            error_message: None,
            is_streaming: false,
        }
    }

    /// Mark the trace as completed
    pub fn complete(&mut self, status_code: u16) {
        self.end_time = Some(Utc::now());
        self.status_code = Some(status_code);
        if let Some(end) = self.end_time {
            self.latency_ms = Some((end - self.start_time).num_milliseconds().max(0) as u64);
        }
    }

    /// Set error information
    pub fn set_error(&mut self, error: String) {
        self.error_message = Some(error);
        self.status_code = Some(500);
        self.complete(500);
    }

    /// Check if the trace was successful
    pub fn is_success(&self) -> bool {
        self.status_code.is_some_and(|s| (200..300).contains(&s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_span_creation() {
        let span = TraceSpan::new(
            "req-123".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            Some("user-456".to_string()),
        );

        assert_eq!(span.request_id, "req-123");
        assert_eq!(span.provider, "openai");
        assert_eq!(span.model, "gpt-4");
        assert_eq!(span.auth_id, Some("user-456".to_string()));
        assert!(span.trace_id != Uuid::nil());
        assert!(span.end_time.is_none());
        assert!(span.status_code.is_none());
    }

    #[tokio::test]
    async fn test_trace_completion() {
        let mut span = TraceSpan::new(
            "req-123".to_string(),
            "anthropic".to_string(),
            "claude-3".to_string(),
            None,
        );

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        span.complete(200);

        assert!(span.end_time.is_some());
        assert_eq!(span.status_code, Some(200));
        assert!(span.latency_ms.is_some());
        assert!(
            span.latency_ms
                .expect("Internal logic invariant should hold")
                >= 9
        );
        assert!(span.is_success());
    }

    #[test]
    fn test_trace_error() {
        let mut span = TraceSpan::new(
            "req-456".to_string(),
            "google".to_string(),
            "gemini-pro".to_string(),
            None,
        );

        span.set_error("Rate limit exceeded".to_string());

        assert_eq!(span.error_message, Some("Rate limit exceeded".to_string()));
        assert_eq!(span.status_code, Some(500));
        assert!(!span.is_success());
    }

    // ===== Edge Case Tests =====

    #[test]
    fn test_complete_zero_status() {
        let mut span = TraceSpan::new(
            "req-zero".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );
        span.complete(0);
        assert_eq!(span.status_code, Some(0));
        assert!(!span.is_success()); // 0 is not in 200..300
        assert!(span.end_time.is_some());
    }

    #[test]
    fn test_complete_large_status() {
        let mut span = TraceSpan::new(
            "req-large".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );
        span.complete(599);
        assert_eq!(span.status_code, Some(599));
        assert!(!span.is_success());
    }

    #[tokio::test]
    async fn test_complete_already_completed() {
        let mut span = TraceSpan::new(
            "req-double".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );
        span.complete(200);
        let _first_end_time = span.end_time;
        let first_latency = span.latency_ms;

        // Small delay to ensure time difference
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;

        span.complete(500);
        // Should update status but end_time and latency should reflect second call
        assert_eq!(span.status_code, Some(500));
        assert!(
            span.latency_ms
                .expect("Internal logic invariant should hold")
                >= first_latency.expect("Internal logic invariant should hold")
        );
    }

    #[test]
    fn test_set_error_empty_message() {
        let mut span = TraceSpan::new(
            "req-empty-err".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );
        span.set_error("".to_string());
        assert_eq!(span.error_message, Some("".to_string()));
        assert_eq!(span.status_code, Some(500));
        assert!(!span.is_success());
    }

    #[test]
    fn test_set_error_unicode_characters() {
        let mut span = TraceSpan::new(
            "req-unicode".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );
        let unicode_error = "错误: 中文错误消息 🚨 エラー";
        span.set_error(unicode_error.to_string());
        assert_eq!(span.error_message, Some(unicode_error.to_string()));
    }

    #[test]
    fn test_is_success_none_status() {
        let span = TraceSpan::new(
            "req-none".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );
        assert!(!span.is_success()); // None should return false
    }

    #[test]
    fn test_is_success_boundary_199() {
        let mut span = TraceSpan::new(
            "req-199".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );
        span.complete(199);
        assert!(!span.is_success()); // 199 is not 2xx
    }

    #[test]
    fn test_is_success_boundary_200() {
        let mut span = TraceSpan::new(
            "req-200".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );
        span.complete(200);
        assert!(span.is_success()); // 200 is 2xx
    }

    #[test]
    fn test_is_success_boundary_299() {
        let mut span = TraceSpan::new(
            "req-299".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );
        span.complete(299);
        assert!(span.is_success()); // 299 is still 2xx
    }

    #[test]
    fn test_is_success_boundary_300() {
        let mut span = TraceSpan::new(
            "req-300".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );
        span.complete(300);
        assert!(!span.is_success()); // 300 is not 2xx
    }
}
