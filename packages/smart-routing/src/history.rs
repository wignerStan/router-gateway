use crate::outcome::ExecutionOutcome;
use crate::statistics::StatisticsAggregator;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Selection mode for route selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SelectionMode {
    /// Weighted random selection
    Weighted,
    /// Thompson sampling (bandit)
    Thompson,
    /// Time-aware selection
    TimeAware,
    /// Quota-aware selection
    QuotaAware,
    /// Adaptive selection
    Adaptive,
    /// Fallback selection
    Fallback,
    /// Manual selection
    Manual,
}

/// Decision context for route selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionContext {
    /// Request ID
    pub request_id: String,
    /// Model ID
    pub model_id: String,
    /// Available route candidates
    pub candidates: Vec<String>,
    /// Selection mode used
    pub selection_mode: SelectionMode,
    /// Predicted utility for each candidate
    pub predicted_utilities: HashMap<String, f64>,
    /// Weights for each candidate
    pub weights: HashMap<String, f64>,
    /// Selected route
    pub selected_route: String,
    /// Timestamp of decision
    pub timestamp: DateTime<Utc>,
    /// Reasoning for selection
    pub reasoning: Option<String>,
}

impl DecisionContext {
    /// Create a new decision context
    pub fn new(
        request_id: String,
        model_id: String,
        candidates: Vec<String>,
        selection_mode: SelectionMode,
        selected_route: String,
    ) -> Self {
        Self {
            request_id,
            model_id,
            candidates,
            selection_mode,
            predicted_utilities: HashMap::new(),
            weights: HashMap::new(),
            selected_route,
            timestamp: Utc::now(),
            reasoning: None,
        }
    }

    /// Set predicted utilities
    pub fn with_predicted_utilities(mut self, utilities: HashMap<String, f64>) -> Self {
        self.predicted_utilities = utilities;
        self
    }

    /// Set weights
    pub fn with_weights(mut self, weights: HashMap<String, f64>) -> Self {
        self.weights = weights;
        self
    }

    /// Set reasoning
    pub fn with_reasoning(mut self, reasoning: String) -> Self {
        self.reasoning = Some(reasoning);
        self
    }

    /// Get predicted utility for a route
    pub fn get_predicted_utility(&self, route_id: &str) -> Option<f64> {
        self.predicted_utilities.get(route_id).copied()
    }

    /// Get weight for a route
    pub fn get_weight(&self, route_id: &str) -> Option<f64> {
        self.weights.get(route_id).copied()
    }
}

/// Attempt information for a route execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteAttempt {
    /// Attempt ID (unique)
    pub attempt_id: String,
    /// Request ID
    pub request_id: String,
    /// Decision context
    pub decision_context: DecisionContext,
    /// Execution outcome
    pub outcome: ExecutionOutcome,
    /// Attempt start time
    pub started_at: DateTime<Utc>,
    /// Attempt completion time
    pub completed_at: DateTime<Utc>,
}

impl RouteAttempt {
    /// Create a new route attempt
    pub fn new(
        request_id: String,
        decision_context: DecisionContext,
        outcome: ExecutionOutcome,
    ) -> Self {
        let started_at = decision_context.timestamp;
        let completed_at = outcome.timestamp;

        // Generate a simple UUID-like string without the uuid crate
        let attempt_id = format!(
            "{}-{}",
            started_at.timestamp(),
            started_at.timestamp_nanos_opt().unwrap_or(0) % 1_000_000
        );

        Self {
            attempt_id,
            request_id,
            decision_context,
            outcome,
            started_at,
            completed_at,
        }
    }

    /// Get attempt duration
    pub fn duration(&self) -> chrono::Duration {
        self.completed_at - self.started_at
    }

    /// Check if attempt was successful
    pub fn is_successful(&self) -> bool {
        self.outcome.success
    }

    /// Check if fallback was used
    pub fn used_fallback(&self) -> bool {
        self.outcome.used_fallback
    }
}

/// Attempt history for analysis and tracing
#[derive(Debug, Clone)]
pub struct AttemptHistory {
    /// History of route attempts
    attempts: Vec<RouteAttempt>,
    /// Maximum attempts to store
    max_attempts: usize,
}

impl AttemptHistory {
    /// Create a new attempt history
    pub fn new() -> Self {
        Self {
            attempts: Vec::new(),
            max_attempts: 100_000,
        }
    }

    /// Create with a limit
    pub fn with_limit(max_attempts: usize) -> Self {
        Self {
            attempts: Vec::new(),
            max_attempts: if max_attempts > 0 {
                max_attempts
            } else {
                100_000
            },
        }
    }

    /// Record a route attempt
    pub fn record(&mut self, attempt: RouteAttempt) {
        self.attempts.push(attempt);

        // Keep only the most recent attempts
        if self.attempts.len() > self.max_attempts {
            let remove_count = self.attempts.len() - self.max_attempts;
            self.attempts.drain(0..remove_count);
        }
    }

    /// Get attempts by request ID
    pub fn get_attempts_for_request(&self, request_id: &str) -> Vec<&RouteAttempt> {
        self.attempts
            .iter()
            .filter(|a| a.request_id == request_id)
            .collect()
    }

    /// Get attempts by route ID
    pub fn get_attempts_for_route(&self, route_id: &str) -> Vec<&RouteAttempt> {
        self.attempts
            .iter()
            .filter(|a| a.outcome.effective_route() == route_id)
            .collect()
    }

    /// Get attempts by model ID
    pub fn get_attempts_for_model(&self, model_id: &str) -> Vec<&RouteAttempt> {
        self.attempts
            .iter()
            .filter(|a| a.decision_context.model_id == model_id)
            .collect()
    }

    /// Get attempts by selection mode
    pub fn get_attempts_by_selection_mode(
        &self,
        mode: &SelectionMode,
    ) -> Vec<&RouteAttempt> {
        self.attempts
            .iter()
            .filter(|a| a.decision_context.selection_mode == *mode)
            .collect()
    }

    /// Get recent attempts (last N)
    pub fn get_recent_attempts(&self, n: usize) -> Vec<&RouteAttempt> {
        let start = if self.attempts.len() > n {
            self.attempts.len() - n
        } else {
            0
        };
        self.attempts[start..].iter().collect()
    }

    /// Get attempts in a time range
    pub fn get_attempts_in_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<&RouteAttempt> {
        self.attempts
            .iter()
            .filter(|a| a.started_at >= start && a.started_at <= end)
            .collect()
    }

    /// Get all attempts
    pub fn get_all_attempts(&self) -> &[RouteAttempt] {
        &self.attempts
    }

    /// Get success rate for a route
    pub fn get_success_rate_for_route(&self, route_id: &str) -> Option<f64> {
        let attempts = self.get_attempts_for_route(route_id);
        if attempts.is_empty() {
            return None;
        }

        let successful = attempts.iter().filter(|a| a.is_successful()).count();
        Some(successful as f64 / attempts.len() as f64)
    }

    /// Get average latency for a route
    pub fn get_avg_latency_for_route(&self, route_id: &str) -> Option<f64> {
        let attempts = self.get_attempts_for_route(route_id);
        if attempts.is_empty() {
            return None;
        }

        let total_latency: f64 = attempts.iter().map(|a| a.outcome.latency_ms).sum();
        Some(total_latency / attempts.len() as f64)
    }

    /// Get fallback usage rate for a route
    pub fn get_fallback_rate_for_route(&self, route_id: &str) -> Option<f64> {
        let attempts = self.get_attempts_for_route(route_id);
        if attempts.is_empty() {
            return None;
        }

        let fallbacks = attempts.iter().filter(|a| a.used_fallback()).count();
        Some(fallbacks as f64 / attempts.len() as f64)
    }

    /// Get selection mode distribution
    pub fn get_selection_mode_distribution(&self) -> HashMap<SelectionMode, usize> {
        let mut distribution = HashMap::new();
        for attempt in &self.attempts {
            *distribution
                .entry(attempt.decision_context.selection_mode.clone())
                .or_insert(0) += 1;
        }
        distribution
    }

    /// Clear all attempts
    pub fn clear(&mut self) {
        self.attempts.clear();
    }

    /// Get the number of attempts
    pub fn len(&self) -> usize {
        self.attempts.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.attempts.is_empty()
    }
}

impl Default for AttemptHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Aggregated metrics from attempt history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptMetrics {
    /// Total attempts
    pub total_attempts: u64,
    /// Successful attempts
    pub successful_attempts: u64,
    /// Failed attempts
    pub failed_attempts: u64,
    /// Fallback attempts
    pub fallback_attempts: u64,
    /// Success rate
    pub success_rate: f64,
    /// Average latency
    pub avg_latency_ms: f64,
    /// P50 latency
    pub p50_latency_ms: f64,
    /// P95 latency
    pub p95_latency_ms: f64,
    /// P99 latency
    pub p99_latency_ms: f64,
    /// Selection mode distribution
    pub selection_mode_distribution: HashMap<String, u64>,
}

impl AttemptHistory {
    /// Calculate metrics for a route
    pub fn calculate_metrics_for_route(&self, route_id: &str) -> Option<AttemptMetrics> {
        let attempts = self.get_attempts_for_route(route_id);
        if attempts.is_empty() {
            return None;
        }

        let total = attempts.len() as u64;
        let successful = attempts.iter().filter(|a| a.is_successful()).count() as u64;
        let failed = total - successful;
        let fallbacks = attempts.iter().filter(|a| a.used_fallback()).count() as u64;

        let mut latencies: Vec<f64> = attempts.iter().map(|a| a.outcome.latency_ms).collect();
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let avg_latency: f64 = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let p50 = latencies[(latencies.len() * 50 / 100).min(latencies.len() - 1)];
        let p95 = latencies[(latencies.len() * 95 / 100).min(latencies.len() - 1)];
        let p99 = latencies[(latencies.len() * 99 / 100).min(latencies.len() - 1)];

        let selection_mode_dist = self
            .get_selection_mode_distribution()
            .into_iter()
            .map(|(k, v)| (format!("{:?}", k), v as u64))
            .collect();

        Some(AttemptMetrics {
            total_attempts: total,
            successful_attempts: successful,
            failed_attempts: failed,
            fallback_attempts: fallbacks,
            success_rate: successful as f64 / total as f64,
            avg_latency_ms: avg_latency,
            p50_latency_ms: p50,
            p95_latency_ms: p95,
            p99_latency_ms: p99,
            selection_mode_distribution: selection_mode_dist,
        })
    }
}

/// Combined tracking system with statistics and history
#[derive(Debug, Clone)]
pub struct TrackingSystem {
    /// Statistics aggregator
    pub statistics: StatisticsAggregator,
    /// Attempt history
    pub history: AttemptHistory,
}

impl TrackingSystem {
    /// Create a new tracking system
    pub fn new() -> Self {
        Self {
            statistics: StatisticsAggregator::new(),
            history: AttemptHistory::new(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        statistics: StatisticsAggregator,
        history: AttemptHistory,
    ) -> Self {
        Self { statistics, history }
    }

    /// Record a route attempt
    pub fn record_attempt(&mut self, attempt: RouteAttempt) {
        // Update statistics
        self.statistics.record(&attempt.outcome);

        // Record in history
        self.history.record(attempt);
    }

    /// Record an execution outcome
    pub fn record_outcome(&mut self, outcome: ExecutionOutcome) {
        self.statistics.record(&outcome);
    }

    /// Get statistics for a route
    pub fn get_statistics(&self, route_id: &str) -> Option<&crate::statistics::RouteStatistics> {
        self.statistics.get_stats(route_id)
    }

    /// Get attempt metrics for a route
    pub fn get_attempt_metrics(&self, route_id: &str) -> Option<AttemptMetrics> {
        self.history.calculate_metrics_for_route(route_id)
    }

    /// Get attempts for a route
    pub fn get_attempts(&self, route_id: &str) -> Vec<&RouteAttempt> {
        self.history.get_attempts_for_route(route_id)
    }

    /// Reset tracking for a route
    pub fn reset_route(&mut self, route_id: &str) {
        self.statistics.reset_route(route_id);
        // Note: We don't clear history as it's for analysis
    }

    /// Reset all tracking
    pub fn reset_all(&mut self) {
        self.statistics.reset_all();
        self.history.clear();
    }
}

impl Default for TrackingSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outcome::ExecutionOutcome;

    #[test]
    fn test_decision_context_creation() {
        let context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["route-1".to_string(), "route-2".to_string()],
            SelectionMode::Weighted,
            "route-1".to_string(),
        );

        assert_eq!(context.request_id, "req-1");
        assert_eq!(context.model_id, "model-1");
        assert_eq!(context.selection_mode, SelectionMode::Weighted);
        assert_eq!(context.selected_route, "route-1");
    }

    #[test]
    fn test_decision_context_with_utilities() {
        let mut utilities = HashMap::new();
        utilities.insert("route-1".to_string(), 0.9);
        utilities.insert("route-2".to_string(), 0.7);

        let context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["route-1".to_string(), "route-2".to_string()],
            SelectionMode::Weighted,
            "route-1".to_string(),
        )
        .with_predicted_utilities(utilities);

        assert_eq!(context.get_predicted_utility("route-1"), Some(0.9));
        assert_eq!(context.get_predicted_utility("route-2"), Some(0.7));
    }

    #[test]
    fn test_route_attempt_creation() {
        let decision_context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["route-1".to_string()],
            SelectionMode::Weighted,
            "route-1".to_string(),
        );

        let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
        let attempt = RouteAttempt::new("req-1".to_string(), decision_context, outcome);

        assert_eq!(attempt.request_id, "req-1");
        assert!(attempt.is_successful());
        assert!(!attempt.used_fallback());
    }

    #[test]
    fn test_attempt_history_record() {
        let mut history = AttemptHistory::new();

        let decision_context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["route-1".to_string()],
            SelectionMode::Weighted,
            "route-1".to_string(),
        );

        let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
        let attempt = RouteAttempt::new("req-1".to_string(), decision_context, outcome);

        history.record(attempt);
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn test_attempt_history_get_attempts_for_route() {
        let mut history = AttemptHistory::new();

        for i in 0..3 {
            let decision_context = DecisionContext::new(
                format!("req-{}", i),
                "model-1".to_string(),
                vec!["route-1".to_string()],
                SelectionMode::Weighted,
                "route-1".to_string(),
            );

            let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
            let attempt = RouteAttempt::new(format!("req-{}", i), decision_context, outcome);
            history.record(attempt);
        }

        let attempts = history.get_attempts_for_route("route-1");
        assert_eq!(attempts.len(), 3);
    }

    #[test]
    fn test_attempt_history_get_success_rate() {
        let mut history = AttemptHistory::new();

        for i in 0..5 {
            let decision_context = DecisionContext::new(
                format!("req-{}", i),
                "model-1".to_string(),
                vec!["route-1".to_string()],
                SelectionMode::Weighted,
                "route-1".to_string(),
            );

            let success = i < 3; // First 3 succeed
            let outcome = if success {
                ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200)
            } else {
                ExecutionOutcome::failure("route-1".to_string(), 200.0, 500, false, None)
            };

            let attempt = RouteAttempt::new(format!("req-{}", i), decision_context, outcome);
            history.record(attempt);
        }

        let success_rate = history.get_success_rate_for_route("route-1").unwrap();
        assert_eq!(success_rate, 0.6); // 3 out of 5
    }

    #[test]
    fn test_attempt_history_get_avg_latency() {
        let mut history = AttemptHistory::new();

        for i in 0..3 {
            let decision_context = DecisionContext::new(
                format!("req-{}", i),
                "model-1".to_string(),
                vec!["route-1".to_string()],
                SelectionMode::Weighted,
                "route-1".to_string(),
            );

            let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0 + i as f64 * 50.0, 10, 5, 200);
            let attempt = RouteAttempt::new(format!("req-{}", i), decision_context, outcome);
            history.record(attempt);
        }

        let avg_latency = history.get_avg_latency_for_route("route-1").unwrap();
        assert_eq!(avg_latency, 150.0); // (100 + 150 + 200) / 3
    }

    #[test]
    fn test_attempt_history_selection_mode_distribution() {
        let mut history = AttemptHistory::new();

        let modes = vec![SelectionMode::Weighted, SelectionMode::Thompson, SelectionMode::Weighted];

        for (i, mode) in modes.iter().enumerate() {
            let decision_context = DecisionContext::new(
                format!("req-{}", i),
                "model-1".to_string(),
                vec!["route-1".to_string()],
                mode.clone(),
                "route-1".to_string(),
            );

            let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
            let attempt = RouteAttempt::new(format!("req-{}", i), decision_context, outcome);
            history.record(attempt);
        }

        let distribution = history.get_selection_mode_distribution();
        assert_eq!(*distribution.get(&SelectionMode::Weighted).unwrap_or(&0), 2);
        assert_eq!(*distribution.get(&SelectionMode::Thompson).unwrap_or(&0), 1);
    }

    #[test]
    fn test_tracking_system() {
        let mut tracking = TrackingSystem::new();

        let decision_context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["route-1".to_string()],
            SelectionMode::Weighted,
            "route-1".to_string(),
        );

        let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
        let attempt = RouteAttempt::new("req-1".to_string(), decision_context, outcome);

        tracking.record_attempt(attempt);

        assert!(tracking.get_statistics("route-1").is_some());
        assert!(tracking.get_attempt_metrics("route-1").is_some());
        assert_eq!(tracking.get_attempts("route-1").len(), 1);
    }

    #[test]
    fn test_tracking_system_reset() {
        let mut tracking = TrackingSystem::new();

        let decision_context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["route-1".to_string()],
            SelectionMode::Weighted,
            "route-1".to_string(),
        );

        let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
        let attempt = RouteAttempt::new("req-1".to_string(), decision_context, outcome.clone());

        tracking.record_attempt(attempt);

        tracking.reset_route("route-1");
        assert!(tracking.get_statistics("route-1").is_none());
        // History should still be available
        assert!(!tracking.get_attempts("route-1").is_empty());
    }
}
