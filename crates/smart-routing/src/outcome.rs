use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Error classification for routing outcomes
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ErrorClass {
    /// Authentication failure (401, 403)
    Auth,
    /// Rate limit exceeded (429)
    RateLimit,
    /// Server error (500, 502, 503, 504)
    ServerError,
    /// Request timeout
    Timeout,
    /// Network error
    Network,
    /// Client error (400, 404, etc.)
    ClientError,
    /// Unknown/other error
    Other,
}

impl ErrorClass {
    /// Classify HTTP status code into error class
    pub fn from_status_code(status_code: i32) -> Option<Self> {
        match status_code {
            401 | 403 => Some(ErrorClass::Auth),
            429 => Some(ErrorClass::RateLimit),
            500 | 502 | 503 | 504 => Some(ErrorClass::ServerError),
            408 => Some(ErrorClass::Timeout),
            400 | 404 | 413 | 422 => Some(ErrorClass::ClientError),
            _ if status_code >= 400 => Some(ErrorClass::Other),
            _ => None, // Success status codes
        }
    }

    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ErrorClass::RateLimit
                | ErrorClass::ServerError
                | ErrorClass::Timeout
                | ErrorClass::Network
        )
    }

    /// Check if error indicates credential issues
    pub fn is_credential_error(&self) -> bool {
        matches!(self, ErrorClass::Auth)
    }
}

/// Execution outcome for a route request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionOutcome {
    /// Route ID that was used
    pub route_id: String,
    /// Whether the request succeeded
    pub success: bool,
    /// Request latency in milliseconds
    pub latency_ms: f64,
    /// Prompt tokens used
    pub prompt_tokens: u32,
    /// Completion tokens generated
    pub completion_tokens: u32,
    /// Total tokens
    pub total_tokens: u32,
    /// Error class (if failed)
    pub error_class: Option<ErrorClass>,
    /// HTTP status code
    pub status_code: Option<i32>,
    /// Timestamp of execution
    pub timestamp: DateTime<Utc>,
    /// Whether fallback was used
    pub used_fallback: bool,
    /// Original route ID (if fallback was used)
    pub original_route_id: Option<String>,
}

impl ExecutionOutcome {
    /// Create a successful outcome
    pub fn success(
        route_id: String,
        latency_ms: f64,
        prompt_tokens: u32,
        completion_tokens: u32,
        status_code: i32,
    ) -> Self {
        Self {
            route_id,
            success: true,
            latency_ms,
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
            error_class: None,
            status_code: Some(status_code),
            timestamp: Utc::now(),
            used_fallback: false,
            original_route_id: None,
        }
    }

    /// Create a failed outcome
    pub fn failure(
        route_id: String,
        latency_ms: f64,
        status_code: i32,
        used_fallback: bool,
        original_route_id: Option<String>,
    ) -> Self {
        let error_class = ErrorClass::from_status_code(status_code);

        Self {
            route_id,
            success: false,
            latency_ms,
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            error_class,
            status_code: Some(status_code),
            timestamp: Utc::now(),
            used_fallback,
            original_route_id,
        }
    }

    /// Create a timeout outcome
    pub fn timeout(route_id: String, latency_ms: f64) -> Self {
        Self {
            route_id,
            success: false,
            latency_ms,
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            error_class: Some(ErrorClass::Timeout),
            status_code: Some(408),
            timestamp: Utc::now(),
            used_fallback: false,
            original_route_id: None,
        }
    }

    /// Create a network error outcome
    pub fn network_error(route_id: String, latency_ms: f64) -> Self {
        Self {
            route_id,
            success: false,
            latency_ms,
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            error_class: Some(ErrorClass::Network),
            status_code: None,
            timestamp: Utc::now(),
            used_fallback: false,
            original_route_id: None,
        }
    }

    /// Get the effective route (original if fallback was used)
    pub fn effective_route(&self) -> &str {
        if self.used_fallback {
            self.original_route_id.as_deref().unwrap_or(&self.route_id)
        } else {
            &self.route_id
        }
    }
}

/// Outcome recorder for tracking route execution results
pub struct OutcomeRecorder {
    outcomes: Vec<ExecutionOutcome>,
    max_outcomes: usize,
}

impl OutcomeRecorder {
    /// Create a new outcome recorder
    pub fn new() -> Self {
        Self {
            outcomes: Vec::new(),
            max_outcomes: 10_000,
        }
    }

    /// Create an outcome recorder with a limit
    pub fn with_limit(max_outcomes: usize) -> Self {
        Self {
            outcomes: Vec::new(),
            max_outcomes: if max_outcomes > 0 {
                max_outcomes
            } else {
                10_000
            },
        }
    }

    /// Record an execution outcome
    pub fn record(&mut self, outcome: ExecutionOutcome) {
        self.outcomes.push(outcome);

        // Keep only the most recent outcomes
        if self.outcomes.len() > self.max_outcomes {
            let remove_count = self.outcomes.len() - self.max_outcomes;
            self.outcomes.drain(0..remove_count);
        }
    }

    /// Record a successful execution
    pub fn record_success(
        &mut self,
        route_id: String,
        latency_ms: f64,
        prompt_tokens: u32,
        completion_tokens: u32,
        status_code: i32,
    ) {
        self.record(ExecutionOutcome::success(
            route_id,
            latency_ms,
            prompt_tokens,
            completion_tokens,
            status_code,
        ));
    }

    /// Record a failed execution
    pub fn record_failure(
        &mut self,
        route_id: String,
        latency_ms: f64,
        status_code: i32,
        used_fallback: bool,
        original_route_id: Option<String>,
    ) {
        self.record(ExecutionOutcome::failure(
            route_id,
            latency_ms,
            status_code,
            used_fallback,
            original_route_id,
        ));
    }

    /// Record a timeout
    pub fn record_timeout(&mut self, route_id: String, latency_ms: f64) {
        self.record(ExecutionOutcome::timeout(route_id, latency_ms));
    }

    /// Record a network error
    pub fn record_network_error(&mut self, route_id: String, latency_ms: f64) {
        self.record(ExecutionOutcome::network_error(route_id, latency_ms));
    }

    /// Get all outcomes for a specific route
    pub fn get_outcomes_for_route(&self, route_id: &str) -> Vec<&ExecutionOutcome> {
        self.outcomes
            .iter()
            .filter(|o| o.effective_route() == route_id)
            .collect()
    }

    /// Get recent outcomes (last N)
    pub fn get_recent_outcomes(&self, n: usize) -> Vec<&ExecutionOutcome> {
        let start = if self.outcomes.len() > n {
            self.outcomes.len() - n
        } else {
            0
        };
        self.outcomes[start..].iter().collect()
    }

    /// Get all outcomes
    pub fn get_all_outcomes(&self) -> &[ExecutionOutcome] {
        &self.outcomes
    }

    /// Clear all outcomes
    pub fn clear(&mut self) {
        self.outcomes.clear();
    }

    /// Get the number of recorded outcomes
    pub fn len(&self) -> usize {
        self.outcomes.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.outcomes.is_empty()
    }
}

impl Default for OutcomeRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for OutcomeRecorder {
    fn clone(&self) -> Self {
        Self {
            outcomes: self.outcomes.clone(),
            max_outcomes: self.max_outcomes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_class_from_status_code() {
        assert_eq!(ErrorClass::from_status_code(401), Some(ErrorClass::Auth));
        assert_eq!(ErrorClass::from_status_code(403), Some(ErrorClass::Auth));
        assert_eq!(
            ErrorClass::from_status_code(429),
            Some(ErrorClass::RateLimit)
        );
        assert_eq!(
            ErrorClass::from_status_code(500),
            Some(ErrorClass::ServerError)
        );
        assert_eq!(
            ErrorClass::from_status_code(502),
            Some(ErrorClass::ServerError)
        );
        assert_eq!(
            ErrorClass::from_status_code(503),
            Some(ErrorClass::ServerError)
        );
        assert_eq!(
            ErrorClass::from_status_code(504),
            Some(ErrorClass::ServerError)
        );
        assert_eq!(ErrorClass::from_status_code(408), Some(ErrorClass::Timeout));
        assert_eq!(
            ErrorClass::from_status_code(400),
            Some(ErrorClass::ClientError)
        );
        assert_eq!(ErrorClass::from_status_code(200), None);
        assert_eq!(ErrorClass::from_status_code(201), None);
    }

    #[test]
    fn test_error_class_is_retryable() {
        assert!(ErrorClass::RateLimit.is_retryable());
        assert!(ErrorClass::ServerError.is_retryable());
        assert!(ErrorClass::Timeout.is_retryable());
        assert!(ErrorClass::Network.is_retryable());
        assert!(!ErrorClass::Auth.is_retryable());
        assert!(!ErrorClass::ClientError.is_retryable());
    }

    #[test]
    fn test_execution_outcome_success() {
        let outcome = ExecutionOutcome::success("route-1".to_string(), 150.0, 100, 50, 200);

        assert!(outcome.success);
        assert_eq!(outcome.route_id, "route-1");
        assert_eq!(outcome.latency_ms, 150.0);
        assert_eq!(outcome.prompt_tokens, 100);
        assert_eq!(outcome.completion_tokens, 50);
        assert_eq!(outcome.total_tokens, 150);
        assert!(outcome.error_class.is_none());
        assert_eq!(outcome.status_code, Some(200));
        assert!(!outcome.used_fallback);
        assert!(outcome.original_route_id.is_none());
    }

    #[test]
    fn test_execution_outcome_failure() {
        let outcome = ExecutionOutcome::failure("route-1".to_string(), 200.0, 500, false, None);

        assert!(!outcome.success);
        assert_eq!(outcome.route_id, "route-1");
        assert_eq!(outcome.latency_ms, 200.0);
        assert_eq!(outcome.error_class, Some(ErrorClass::ServerError));
        assert_eq!(outcome.status_code, Some(500));
        assert!(!outcome.used_fallback);
        assert!(outcome.original_route_id.is_none());
    }

    #[test]
    fn test_execution_outcome_with_fallback() {
        let outcome = ExecutionOutcome::failure(
            "route-fallback".to_string(),
            300.0,
            200,
            true,
            Some("route-original".to_string()),
        );

        assert!(!outcome.success);
        assert_eq!(outcome.route_id, "route-fallback");
        assert!(outcome.used_fallback);
        assert_eq!(
            outcome.original_route_id,
            Some("route-original".to_string())
        );
        assert_eq!(outcome.effective_route(), "route-original");
    }

    #[test]
    fn test_outcome_recorder_record() {
        let mut recorder = OutcomeRecorder::new();

        recorder.record_success("route-1".to_string(), 100.0, 10, 5, 200);
        recorder.record_failure("route-2".to_string(), 200.0, 500, false, None);

        assert_eq!(recorder.len(), 2);
    }

    #[test]
    fn test_outcome_recorder_get_outcomes_for_route() {
        let mut recorder = OutcomeRecorder::new();

        recorder.record_success("route-1".to_string(), 100.0, 10, 5, 200);
        recorder.record_success("route-1".to_string(), 150.0, 20, 10, 200);
        recorder.record_failure("route-2".to_string(), 200.0, 500, false, None);

        let route1_outcomes = recorder.get_outcomes_for_route("route-1");
        assert_eq!(route1_outcomes.len(), 2);

        let route2_outcomes = recorder.get_outcomes_for_route("route-2");
        assert_eq!(route2_outcomes.len(), 1);
    }

    #[test]
    fn test_outcome_recorder_recent_outcomes() {
        let mut recorder = OutcomeRecorder::new();

        for i in 0..10 {
            recorder.record_success(format!("route-{}", i), 100.0, 10, 5, 200);
        }

        let recent = recorder.get_recent_outcomes(5);
        assert_eq!(recent.len(), 5);
    }

    #[test]
    fn test_outcome_recorder_limit() {
        let mut recorder = OutcomeRecorder::with_limit(5);

        for i in 0..10 {
            recorder.record_success(format!("route-{}", i), 100.0, 10, 5, 200);
        }

        // Should only keep the last 5
        assert_eq!(recorder.len(), 5);

        // First 5 should be removed
        assert!(recorder.get_outcomes_for_route("route-0").is_empty());
        assert!(recorder.get_outcomes_for_route("route-4").is_empty());
        assert!(!recorder.get_outcomes_for_route("route-5").is_empty());
        assert!(!recorder.get_outcomes_for_route("route-9").is_empty());
    }

    #[test]
    fn test_outcome_recorder_clear() {
        let mut recorder = OutcomeRecorder::new();

        recorder.record_success("route-1".to_string(), 100.0, 10, 5, 200);
        assert_eq!(recorder.len(), 1);

        recorder.clear();
        assert!(recorder.is_empty());
    }

    #[test]
    fn test_outcome_recorder_clone() {
        let mut recorder1 = OutcomeRecorder::new();
        recorder1.record_success("route-1".to_string(), 100.0, 10, 5, 200);

        let mut recorder2 = recorder1.clone();
        recorder2.record_success("route-2".to_string(), 150.0, 20, 10, 200);

        // recorder1 should not be affected by recorder2
        assert_eq!(recorder1.len(), 1);
        assert_eq!(recorder2.len(), 2);
    }
}
