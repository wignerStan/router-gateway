#![allow(clippy::float_cmp)]
use crate::routing::outcome::ExecutionOutcome;
use crate::routing::statistics::StatisticsAggregator;
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
    #[must_use]
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
    #[must_use]
    pub fn with_predicted_utilities(mut self, utilities: HashMap<String, f64>) -> Self {
        self.predicted_utilities = utilities;
        self
    }

    /// Set weights
    #[must_use]
    pub fn with_weights(mut self, weights: HashMap<String, f64>) -> Self {
        self.weights = weights;
        self
    }

    /// Set reasoning
    #[must_use]
    pub fn with_reasoning(mut self, reasoning: String) -> Self {
        self.reasoning = Some(reasoning);
        self
    }

    /// Get predicted utility for a route
    #[must_use]
    pub fn get_predicted_utility(&self, route_id: &str) -> Option<f64> {
        self.predicted_utilities.get(route_id).copied()
    }

    /// Get weight for a route
    #[must_use]
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
    #[must_use]
    pub fn new(
        request_id: String,
        decision_context: DecisionContext,
        outcome: ExecutionOutcome,
    ) -> Self {
        let started_at = decision_context.timestamp;
        let completed_at = outcome.timestamp;

        // Ensure started_at <= completed_at
        let (started_at, completed_at) = if started_at <= completed_at {
            (started_at, completed_at)
        } else {
            (completed_at, started_at)
        };

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
    #[must_use]
    pub fn duration(&self) -> chrono::Duration {
        self.completed_at - self.started_at
    }

    /// Check if attempt was successful
    #[must_use]
    pub const fn is_successful(&self) -> bool {
        self.outcome.success
    }

    /// Check if fallback was used
    #[must_use]
    pub const fn used_fallback(&self) -> bool {
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
    #[must_use]
    pub const fn new() -> Self {
        Self {
            attempts: Vec::new(),
            max_attempts: 100_000,
        }
    }

    /// Create with a limit
    #[must_use]
    pub const fn with_limit(max_attempts: usize) -> Self {
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
        if self.max_attempts == 0 {
            return;
        }

        self.attempts.push(attempt);

        if self.attempts.len() > self.max_attempts {
            let remove_count = self.attempts.len() - self.max_attempts;
            self.attempts.drain(0..remove_count);
        }
    }

    /// Get attempts by request ID
    #[must_use]
    pub fn get_attempts_for_request(&self, request_id: &str) -> Vec<&RouteAttempt> {
        self.attempts
            .iter()
            .filter(|a| a.request_id == request_id)
            .collect()
    }

    /// Get attempts by route ID
    #[must_use]
    pub fn get_attempts_for_route(&self, route_id: &str) -> Vec<&RouteAttempt> {
        self.attempts
            .iter()
            .filter(|a| a.outcome.effective_route() == route_id)
            .collect()
    }

    /// Get attempts by model ID
    #[must_use]
    pub fn get_attempts_for_model(&self, model_id: &str) -> Vec<&RouteAttempt> {
        self.attempts
            .iter()
            .filter(|a| a.decision_context.model_id == model_id)
            .collect()
    }

    /// Get attempts by selection mode
    #[must_use]
    pub fn get_attempts_by_selection_mode(&self, mode: &SelectionMode) -> Vec<&RouteAttempt> {
        self.attempts
            .iter()
            .filter(|a| a.decision_context.selection_mode == *mode)
            .collect()
    }

    /// Get recent attempts (last N)
    #[must_use]
    pub fn get_recent_attempts(&self, n: usize) -> Vec<&RouteAttempt> {
        let start = if self.attempts.len() > n {
            self.attempts.len() - n
        } else {
            0
        };
        self.attempts[start..].iter().collect()
    }

    /// Get attempts in a time range
    #[must_use]
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
    #[must_use]
    pub fn get_all_attempts(&self) -> &[RouteAttempt] {
        &self.attempts
    }

    /// Get success rate for a route
    #[must_use]
    pub fn get_success_rate_for_route(&self, route_id: &str) -> Option<f64> {
        let attempts = self.get_attempts_for_route(route_id);
        if attempts.is_empty() {
            return None;
        }

        let successful = attempts.iter().filter(|a| a.is_successful()).count();
        Some(successful as f64 / attempts.len() as f64)
    }

    /// Get average latency for a route
    #[must_use]
    pub fn get_avg_latency_for_route(&self, route_id: &str) -> Option<f64> {
        let attempts = self.get_attempts_for_route(route_id);
        if attempts.is_empty() {
            return None;
        }

        let total_latency: f64 = attempts.iter().map(|a| a.outcome.latency_ms).sum();
        Some(total_latency / attempts.len() as f64)
    }

    /// Get fallback usage rate for a route
    #[must_use]
    pub fn get_fallback_rate_for_route(&self, route_id: &str) -> Option<f64> {
        let attempts = self.get_attempts_for_route(route_id);
        if attempts.is_empty() {
            return None;
        }

        let fallbacks = attempts.iter().filter(|a| a.used_fallback()).count();
        Some(fallbacks as f64 / attempts.len() as f64)
    }

    /// Get selection mode distribution
    #[must_use]
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
    #[must_use]
    pub fn len(&self) -> usize {
        self.attempts.len()
    }

    /// Check if empty
    #[must_use]
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
    #[must_use]
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
        latencies.sort_by(f64::total_cmp);

        let avg_latency: f64 = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let p50 = latencies[(latencies.len() * 50 / 100).min(latencies.len() - 1)];
        let p95 = latencies[(latencies.len() * 95 / 100).min(latencies.len() - 1)];
        let p99 = latencies[(latencies.len() * 99 / 100).min(latencies.len() - 1)];

        let selection_mode_dist = self
            .get_selection_mode_distribution()
            .into_iter()
            .map(|(k, v)| (format!("{k:?}"), v as u64))
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
    #[must_use]
    pub fn new() -> Self {
        Self {
            statistics: StatisticsAggregator::new(),
            history: AttemptHistory::new(),
        }
    }

    /// Create with custom configuration
    #[must_use]
    pub const fn with_config(statistics: StatisticsAggregator, history: AttemptHistory) -> Self {
        Self {
            statistics,
            history,
        }
    }

    /// Record a route attempt
    pub fn record_attempt(&mut self, attempt: RouteAttempt) {
        self.statistics.record(&attempt.outcome);
        self.history.record(attempt);
    }

    /// Record an execution outcome
    pub fn record_outcome(&mut self, outcome: &ExecutionOutcome) {
        self.statistics.record(outcome);
    }

    /// Get statistics for a route
    #[must_use]
    pub fn get_statistics(
        &self,
        route_id: &str,
    ) -> Option<&crate::routing::statistics::RouteStatistics> {
        self.statistics.get_stats(route_id)
    }

    /// Get attempt metrics for a route
    #[must_use]
    pub fn get_attempt_metrics(&self, route_id: &str) -> Option<AttemptMetrics> {
        self.history.calculate_metrics_for_route(route_id)
    }

    /// Get attempts for a route
    #[must_use]
    pub fn get_attempts(&self, route_id: &str) -> Vec<&RouteAttempt> {
        self.history.get_attempts_for_route(route_id)
    }

    /// Reset tracking for a route
    pub fn reset_route(&mut self, route_id: &str) {
        self.statistics.reset_route(route_id);
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
    use crate::routing::outcome::ExecutionOutcome;

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
                format!("req-{i}"),
                "model-1".to_string(),
                vec!["route-1".to_string()],
                SelectionMode::Weighted,
                "route-1".to_string(),
            );

            let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
            let attempt = RouteAttempt::new(format!("req-{i}"), decision_context, outcome);
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
                format!("req-{i}"),
                "model-1".to_string(),
                vec!["route-1".to_string()],
                SelectionMode::Weighted,
                "route-1".to_string(),
            );

            let success = i < 3;
            let outcome = if success {
                ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200)
            } else {
                ExecutionOutcome::failure("route-1".to_string(), 200.0, 500, false, None)
            };

            let attempt = RouteAttempt::new(format!("req-{i}"), decision_context, outcome);
            history.record(attempt);
        }

        let success_rate = history.get_success_rate_for_route("route-1").unwrap();
        assert_eq!(success_rate, 0.6);
    }

    #[test]
    fn test_attempt_history_get_avg_latency() {
        let mut history = AttemptHistory::new();

        for i in 0..3 {
            let decision_context = DecisionContext::new(
                format!("req-{i}"),
                "model-1".to_string(),
                vec!["route-1".to_string()],
                SelectionMode::Weighted,
                "route-1".to_string(),
            );

            let outcome = ExecutionOutcome::success(
                "route-1".to_string(),
                f64::from(i).mul_add(50.0, 100.0),
                10,
                5,
                200,
            );
            let attempt = RouteAttempt::new(format!("req-{i}"), decision_context, outcome);
            history.record(attempt);
        }

        let avg_latency = history.get_avg_latency_for_route("route-1").unwrap();
        assert_eq!(avg_latency, 150.0);
    }

    #[test]
    fn test_attempt_history_selection_mode_distribution() {
        let mut history = AttemptHistory::new();

        let modes = [
            SelectionMode::Weighted,
            SelectionMode::Thompson,
            SelectionMode::Weighted,
        ];

        for (i, mode) in modes.iter().enumerate() {
            let decision_context = DecisionContext::new(
                format!("req-{i}"),
                "model-1".to_string(),
                vec!["route-1".to_string()],
                mode.clone(),
                "route-1".to_string(),
            );

            let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
            let attempt = RouteAttempt::new(format!("req-{i}"), decision_context, outcome);
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
        let attempt = RouteAttempt::new("req-1".to_string(), decision_context, outcome);

        tracking.record_attempt(attempt);

        tracking.reset_route("route-1");
        assert!(tracking.get_statistics("route-1").is_none());
        assert!(!tracking.get_attempts("route-1").is_empty());
    }

    // --- DecisionContext builder methods ---

    #[test]
    fn test_decision_context_with_weights() {
        let mut weights = HashMap::new();
        weights.insert("route-1".to_string(), 0.8);
        weights.insert("route-2".to_string(), 0.6);

        let context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["route-1".to_string(), "route-2".to_string()],
            SelectionMode::Adaptive,
            "route-1".to_string(),
        )
        .with_weights(weights);

        pretty_assertions::assert_eq!(context.get_weight("route-1"), Some(0.8));
        pretty_assertions::assert_eq!(context.get_weight("route-2"), Some(0.6));
        pretty_assertions::assert_eq!(context.get_weight("route-999"), None);
    }

    #[test]
    fn test_decision_context_with_reasoning() {
        let context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["route-1".to_string()],
            SelectionMode::Manual,
            "route-1".to_string(),
        )
        .with_reasoning("User override".to_string());

        pretty_assertions::assert_eq!(context.reasoning, Some("User override".to_string()));
    }

    #[test]
    fn test_decision_context_no_reasoning_by_default() {
        let context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["route-1".to_string()],
            SelectionMode::Weighted,
            "route-1".to_string(),
        );

        assert!(context.reasoning.is_none());
    }

    #[test]
    fn test_decision_context_get_predicted_utility_missing() {
        let context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["route-1".to_string()],
            SelectionMode::Weighted,
            "route-1".to_string(),
        );

        pretty_assertions::assert_eq!(context.get_predicted_utility("nonexistent"), None);
    }

    #[test]
    fn test_decision_context_builder_chain() {
        let mut utilities = HashMap::new();
        utilities.insert("route-1".to_string(), 0.95);

        let mut weights = HashMap::new();
        weights.insert("route-1".to_string(), 0.7);

        let context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["route-1".to_string()],
            SelectionMode::Thompson,
            "route-1".to_string(),
        )
        .with_predicted_utilities(utilities)
        .with_weights(weights)
        .with_reasoning("Highest utility".to_string());

        pretty_assertions::assert_eq!(context.get_predicted_utility("route-1"), Some(0.95));
        pretty_assertions::assert_eq!(context.get_weight("route-1"), Some(0.7));
        pretty_assertions::assert_eq!(context.reasoning, Some("Highest utility".to_string()));
        pretty_assertions::assert_eq!(context.selection_mode, SelectionMode::Thompson);
    }

    // --- SelectionMode variants ---

    #[test]
    fn test_selection_mode_equality_and_hash() {
        let mode1 = SelectionMode::Weighted;
        let mode2 = SelectionMode::Weighted;
        let mode3 = SelectionMode::Thompson;

        assert_eq!(mode1, mode2);
        assert_ne!(mode1, mode3);

        let mut map = HashMap::new();
        map.insert(mode1, 1);
        map.insert(mode3, 2);
        pretty_assertions::assert_eq!(map.len(), 2);
        pretty_assertions::assert_eq!(map.get(&mode2), Some(&1));
    }

    // --- RouteAttempt ---

    #[test]
    fn test_route_attempt_duration() {
        let decision_context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["route-1".to_string()],
            SelectionMode::Weighted,
            "route-1".to_string(),
        );

        let outcome = ExecutionOutcome::success("route-1".to_string(), 250.0, 10, 5, 200);
        let attempt = RouteAttempt::new("req-1".to_string(), decision_context, outcome);

        // Duration should be non-negative (outcome timestamp >= decision timestamp)
        let duration = attempt.duration();
        assert!(
            duration.num_milliseconds() >= 0,
            "Duration should be non-negative"
        );
    }

    #[test]
    fn test_route_attempt_used_fallback_true() {
        let decision_context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["route-1".to_string()],
            SelectionMode::Fallback,
            "route-1".to_string(),
        );

        let outcome = ExecutionOutcome::failure(
            "route-1".to_string(),
            200.0,
            500,
            true,
            Some("route-original".to_string()),
        );
        let attempt = RouteAttempt::new("req-1".to_string(), decision_context, outcome);

        assert!(attempt.used_fallback());
        assert!(!attempt.is_successful());
    }

    #[test]
    fn test_route_attempt_attempt_id_format() {
        let decision_context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["route-1".to_string()],
            SelectionMode::Weighted,
            "route-1".to_string(),
        );

        let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
        let attempt = RouteAttempt::new("req-1".to_string(), decision_context, outcome);

        // Attempt ID should be "timestamp-nanos_remainder" format
        assert!(
            attempt.attempt_id.contains('-'),
            "Attempt ID should contain a dash: {}",
            attempt.attempt_id
        );
    }

    // --- AttemptHistory with_limit and edge cases ---

    #[test]
    fn test_attempt_history_with_limit_zero_uses_default() {
        let history = AttemptHistory::with_limit(0);
        // Should default to 100_000 when zero is passed
        pretty_assertions::assert_eq!(history.max_attempts, 100_000);
    }

    #[test]
    fn test_attempt_history_with_limit_custom() {
        let history = AttemptHistory::with_limit(50);
        pretty_assertions::assert_eq!(history.max_attempts, 50);
    }

    #[test]
    fn test_attempt_history_eviction_when_over_limit() {
        let mut history = AttemptHistory::with_limit(3);

        for i in 0..5 {
            let decision_context = DecisionContext::new(
                format!("req-{i}"),
                "model-1".to_string(),
                vec!["route-1".to_string()],
                SelectionMode::Weighted,
                "route-1".to_string(),
            );
            let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
            let attempt = RouteAttempt::new(format!("req-{i}"), decision_context, outcome);
            history.record(attempt);
        }

        // Should only keep the last 3 (req-2, req-3, req-4)
        pretty_assertions::assert_eq!(history.len(), 3);

        let attempts = history.get_all_attempts();
        pretty_assertions::assert_eq!(attempts[0].request_id, "req-2");
        pretty_assertions::assert_eq!(attempts[1].request_id, "req-3");
        pretty_assertions::assert_eq!(attempts[2].request_id, "req-4");
    }

    #[test]
    fn test_attempt_history_eviction_drains_multiple() {
        let mut history = AttemptHistory::with_limit(2);

        for i in 0..10 {
            let decision_context = DecisionContext::new(
                format!("req-{i}"),
                "model-1".to_string(),
                vec!["route-1".to_string()],
                SelectionMode::Weighted,
                "route-1".to_string(),
            );
            let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
            let attempt = RouteAttempt::new(format!("req-{i}"), decision_context, outcome);
            history.record(attempt);
        }

        pretty_assertions::assert_eq!(history.len(), 2);
        let attempts = history.get_all_attempts();
        pretty_assertions::assert_eq!(attempts[0].request_id, "req-8");
        pretty_assertions::assert_eq!(attempts[1].request_id, "req-9");
    }

    #[test]
    fn test_attempt_history_default() {
        let history = AttemptHistory::default();
        assert!(history.is_empty());
        pretty_assertions::assert_eq!(history.len(), 0);
    }

    // --- get_attempts_for_request ---

    #[test]
    fn test_get_attempts_for_request_found() {
        let mut history = AttemptHistory::new();

        // Record attempts with different request IDs
        for req_id in &["req-a", "req-b", "req-a"] {
            let decision_context = DecisionContext::new(
                req_id.to_string(),
                "model-1".to_string(),
                vec!["route-1".to_string()],
                SelectionMode::Weighted,
                "route-1".to_string(),
            );
            let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
            let attempt = RouteAttempt::new(req_id.to_string(), decision_context, outcome);
            history.record(attempt);
        }

        let results = history.get_attempts_for_request("req-a");
        pretty_assertions::assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_get_attempts_for_request_not_found() {
        let history = AttemptHistory::new();
        let results = history.get_attempts_for_request("nonexistent");
        assert!(results.is_empty());
    }

    // --- get_attempts_for_model ---

    #[test]
    fn test_get_attempts_for_model_found() {
        let mut history = AttemptHistory::new();

        for (i, model) in ["gpt-4", "claude-3", "gpt-4"].iter().enumerate() {
            let decision_context = DecisionContext::new(
                format!("req-{i}"),
                model.to_string(),
                vec!["route-1".to_string()],
                SelectionMode::Weighted,
                "route-1".to_string(),
            );
            let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
            let attempt = RouteAttempt::new(format!("req-{i}"), decision_context, outcome);
            history.record(attempt);
        }

        let results = history.get_attempts_for_model("gpt-4");
        pretty_assertions::assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_get_attempts_for_model_not_found() {
        let history = AttemptHistory::new();
        let results = history.get_attempts_for_model("nonexistent");
        assert!(results.is_empty());
    }

    // --- get_attempts_by_selection_mode ---

    #[test]
    fn test_get_attempts_by_selection_mode_found() {
        let mut history = AttemptHistory::new();

        let modes = [
            SelectionMode::Weighted,
            SelectionMode::Thompson,
            SelectionMode::TimeAware,
            SelectionMode::Weighted,
        ];

        for (i, mode) in modes.iter().enumerate() {
            let decision_context = DecisionContext::new(
                format!("req-{i}"),
                "model-1".to_string(),
                vec!["route-1".to_string()],
                mode.clone(),
                "route-1".to_string(),
            );
            let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
            let attempt = RouteAttempt::new(format!("req-{i}"), decision_context, outcome);
            history.record(attempt);
        }

        let weighted = history.get_attempts_by_selection_mode(&SelectionMode::Weighted);
        pretty_assertions::assert_eq!(weighted.len(), 2);

        let thompson = history.get_attempts_by_selection_mode(&SelectionMode::Thompson);
        pretty_assertions::assert_eq!(thompson.len(), 1);

        let quota = history.get_attempts_by_selection_mode(&SelectionMode::QuotaAware);
        assert!(quota.is_empty());
    }

    // --- get_recent_attempts edge cases ---

    #[test]
    fn test_get_recent_attempts_n_greater_than_len() {
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

        // Request more than stored
        let recent = history.get_recent_attempts(10);
        pretty_assertions::assert_eq!(recent.len(), 1);
    }

    #[test]
    fn test_get_recent_attempts_n_zero() {
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

        let recent = history.get_recent_attempts(0);
        assert!(recent.is_empty());
    }

    #[test]
    fn test_get_recent_attempts_empty_history() {
        let history = AttemptHistory::new();
        let recent = history.get_recent_attempts(5);
        assert!(recent.is_empty());
    }

    #[test]
    fn test_get_recent_attempts_returns_newest() {
        let mut history = AttemptHistory::new();

        for i in 0..5 {
            let decision_context = DecisionContext::new(
                format!("req-{i}"),
                "model-1".to_string(),
                vec!["route-1".to_string()],
                SelectionMode::Weighted,
                "route-1".to_string(),
            );
            let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
            let attempt = RouteAttempt::new(format!("req-{i}"), decision_context, outcome);
            history.record(attempt);
        }

        let recent = history.get_recent_attempts(2);
        pretty_assertions::assert_eq!(recent.len(), 2);
        pretty_assertions::assert_eq!(recent[0].request_id, "req-3");
        pretty_assertions::assert_eq!(recent[1].request_id, "req-4");
    }

    // --- get_attempts_in_range ---

    #[test]
    fn test_get_attempts_in_range_found() {
        let mut history = AttemptHistory::new();
        let now = Utc::now();
        let _one_hour_ago = now - chrono::Duration::hours(1);
        let two_hours_ago = now - chrono::Duration::hours(2);

        // Create an attempt whose started_at will be based on decision_context.timestamp
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

        // Use a wide range that should include the recent attempt
        let results =
            history.get_attempts_in_range(two_hours_ago, now + chrono::Duration::hours(1));
        pretty_assertions::assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_get_attempts_in_range_none_match() {
        let mut history = AttemptHistory::new();
        let now = Utc::now();
        let far_future = now + chrono::Duration::days(365);

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

        // Range far in the future should find nothing
        let results =
            history.get_attempts_in_range(far_future, far_future + chrono::Duration::hours(1));
        assert!(results.is_empty());
    }

    #[test]
    fn test_get_attempts_in_range_empty_history() {
        let history = AttemptHistory::new();
        let now = Utc::now();
        let results = history.get_attempts_in_range(now, now);
        assert!(results.is_empty());
    }

    // --- get_all_attempts and clear ---

    #[test]
    fn test_get_all_attempts() {
        let mut history = AttemptHistory::new();

        for i in 0..3 {
            let decision_context = DecisionContext::new(
                format!("req-{i}"),
                "model-1".to_string(),
                vec!["route-1".to_string()],
                SelectionMode::Weighted,
                "route-1".to_string(),
            );
            let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
            let attempt = RouteAttempt::new(format!("req-{i}"), decision_context, outcome);
            history.record(attempt);
        }

        let all = history.get_all_attempts();
        pretty_assertions::assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_clear_empties_history() {
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

        pretty_assertions::assert_eq!(history.len(), 1);
        history.clear();
        assert!(history.is_empty());
        pretty_assertions::assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_is_empty_on_new_history() {
        let history = AttemptHistory::new();
        assert!(history.is_empty());
    }

    // --- get_success_rate_for_route returning None ---

    #[test]
    fn test_get_success_rate_for_route_none_when_empty() {
        let history = AttemptHistory::new();
        assert!(history.get_success_rate_for_route("route-1").is_none());
    }

    #[test]
    fn test_get_success_rate_for_route_none_when_route_not_present() {
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

        assert!(history.get_success_rate_for_route("route-999").is_none());
    }

    // --- get_avg_latency_for_route returning None ---

    #[test]
    fn test_get_avg_latency_for_route_none_when_empty() {
        let history = AttemptHistory::new();
        assert!(history.get_avg_latency_for_route("route-1").is_none());
    }

    #[test]
    fn test_get_avg_latency_for_route_none_when_route_not_present() {
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

        assert!(history.get_avg_latency_for_route("route-999").is_none());
    }

    // --- get_fallback_rate_for_route ---

    #[test]
    fn test_get_fallback_rate_for_route_none_when_empty() {
        let history = AttemptHistory::new();
        assert!(history.get_fallback_rate_for_route("route-1").is_none());
    }

    #[test]
    fn test_get_fallback_rate_for_route_with_fallbacks() {
        let mut history = AttemptHistory::new();

        // 2 attempts with fallback, 1 without
        for i in 0..3 {
            let decision_context = DecisionContext::new(
                format!("req-{i}"),
                "model-1".to_string(),
                vec!["route-1".to_string()],
                SelectionMode::Fallback,
                "route-1".to_string(),
            );

            let used_fallback = i < 2;
            let outcome = if used_fallback {
                ExecutionOutcome::failure(
                    "route-1".to_string(),
                    200.0,
                    500,
                    true,
                    Some("route-original".to_string()),
                )
            } else {
                ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200)
            };

            let attempt = RouteAttempt::new(format!("req-{i}"), decision_context, outcome);
            history.record(attempt);
        }

        let fallback_rate = history
            .get_fallback_rate_for_route("route-original")
            .expect("should have a fallback rate");
        // 2 out of 3 used fallback, but effective_route for fallback attempts is "route-original"
        // For the non-fallback success, effective_route is "route-1"
        pretty_assertions::assert_eq!(fallback_rate, 1.0);
    }

    #[test]
    fn test_get_fallback_rate_for_route_no_fallbacks() {
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

        let fallback_rate = history
            .get_fallback_rate_for_route("route-1")
            .expect("should have a fallback rate");
        pretty_assertions::assert_eq!(fallback_rate, 0.0);
    }

    // --- calculate_metrics_for_route ---

    #[test]
    fn test_calculate_metrics_for_route_none_when_empty() {
        let history = AttemptHistory::new();
        assert!(history.calculate_metrics_for_route("route-1").is_none());
    }

    #[test]
    fn test_calculate_metrics_single_entry() {
        let mut history = AttemptHistory::new();

        let decision_context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["route-1".to_string()],
            SelectionMode::Weighted,
            "route-1".to_string(),
        );
        let outcome = ExecutionOutcome::success("route-1".to_string(), 150.0, 10, 5, 200);
        let attempt = RouteAttempt::new("req-1".to_string(), decision_context, outcome);
        history.record(attempt);

        let metrics = history
            .calculate_metrics_for_route("route-1")
            .expect("should have metrics");

        pretty_assertions::assert_eq!(metrics.total_attempts, 1);
        pretty_assertions::assert_eq!(metrics.successful_attempts, 1);
        pretty_assertions::assert_eq!(metrics.failed_attempts, 0);
        pretty_assertions::assert_eq!(metrics.fallback_attempts, 0);
        pretty_assertions::assert_eq!(metrics.success_rate, 1.0);
        // With one entry, all percentiles equal the single latency
        pretty_assertions::assert_eq!(metrics.avg_latency_ms, 150.0);
        pretty_assertions::assert_eq!(metrics.p50_latency_ms, 150.0);
        pretty_assertions::assert_eq!(metrics.p95_latency_ms, 150.0);
        pretty_assertions::assert_eq!(metrics.p99_latency_ms, 150.0);
    }

    #[test]
    fn test_calculate_metrics_mixed_outcomes() {
        let mut history = AttemptHistory::new();

        // Record 4 successes and 1 failure for route-1
        let outcomes: Vec<ExecutionOutcome> = vec![
            ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200),
            ExecutionOutcome::success("route-1".to_string(), 200.0, 10, 5, 200),
            ExecutionOutcome::success("route-1".to_string(), 300.0, 10, 5, 200),
            ExecutionOutcome::success("route-1".to_string(), 400.0, 10, 5, 200),
            ExecutionOutcome::failure("route-1".to_string(), 500.0, 500, false, None),
        ];

        for (i, outcome) in outcomes.into_iter().enumerate() {
            let decision_context = DecisionContext::new(
                format!("req-{i}"),
                "model-1".to_string(),
                vec!["route-1".to_string()],
                if i % 2 == 0 {
                    SelectionMode::Weighted
                } else {
                    SelectionMode::Thompson
                },
                "route-1".to_string(),
            );
            let attempt = RouteAttempt::new(format!("req-{i}"), decision_context, outcome);
            history.record(attempt);
        }

        let metrics = history
            .calculate_metrics_for_route("route-1")
            .expect("should have metrics");

        pretty_assertions::assert_eq!(metrics.total_attempts, 5);
        pretty_assertions::assert_eq!(metrics.successful_attempts, 4);
        pretty_assertions::assert_eq!(metrics.failed_attempts, 1);
        pretty_assertions::assert_eq!(metrics.success_rate, 0.8);

        // Latencies sorted: [100, 200, 300, 400, 500]
        // avg = 300.0
        let expected_avg = (100.0 + 200.0 + 300.0 + 400.0 + 500.0) / 5.0;
        pretty_assertions::assert_eq!(metrics.avg_latency_ms, expected_avg);

        // p50: index = (5 * 50 / 100).min(4) = 2.min(4) = 2 -> 300.0
        pretty_assertions::assert_eq!(metrics.p50_latency_ms, 300.0);
        // p95: index = (5 * 95 / 100).min(4) = 4.min(4) = 4 -> 500.0
        pretty_assertions::assert_eq!(metrics.p95_latency_ms, 500.0);
        // p99: index = (5 * 99 / 100).min(4) = 4.min(4) = 4 -> 500.0
        pretty_assertions::assert_eq!(metrics.p99_latency_ms, 500.0);
    }

    #[test]
    fn test_calculate_metrics_selection_mode_distribution_strings() {
        let mut history = AttemptHistory::new();

        let decision_context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["route-1".to_string()],
            SelectionMode::QuotaAware,
            "route-1".to_string(),
        );
        let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
        let attempt = RouteAttempt::new("req-1".to_string(), decision_context, outcome);
        history.record(attempt);

        let metrics = history
            .calculate_metrics_for_route("route-1")
            .expect("should have metrics");

        // Selection mode distribution keys should be Debug-formatted strings
        let expected_key = format!("{:?}", SelectionMode::QuotaAware);
        pretty_assertions::assert_eq!(
            metrics.selection_mode_distribution.get(&expected_key),
            Some(&1)
        );
    }

    // --- Multiple providers filtering ---

    #[test]
    fn test_multiple_providers_filtered_correctly() {
        let mut history = AttemptHistory::new();

        // Record attempts for route-1, route-2, and route-3
        for (i, route) in ["route-1", "route-2", "route-1", "route-3", "route-2"]
            .iter()
            .enumerate()
        {
            let decision_context = DecisionContext::new(
                format!("req-{i}"),
                "model-1".to_string(),
                vec![route.to_string()],
                SelectionMode::Weighted,
                route.to_string(),
            );
            let outcome = ExecutionOutcome::success(route.to_string(), 100.0, 10, 5, 200);
            let attempt = RouteAttempt::new(format!("req-{i}"), decision_context, outcome);
            history.record(attempt);
        }

        pretty_assertions::assert_eq!(history.len(), 5);

        let route1 = history.get_attempts_for_route("route-1");
        let route2 = history.get_attempts_for_route("route-2");
        let route3 = history.get_attempts_for_route("route-3");
        let route_none = history.get_attempts_for_route("route-999");

        pretty_assertions::assert_eq!(route1.len(), 2);
        pretty_assertions::assert_eq!(route2.len(), 2);
        pretty_assertions::assert_eq!(route3.len(), 1);
        assert!(route_none.is_empty());
    }

    // --- TrackingSystem ---

    #[test]
    fn test_tracking_system_default() {
        let tracking = TrackingSystem::default();
        assert!(tracking.history.is_empty());
    }

    #[test]
    fn test_tracking_system_with_config() {
        let history = AttemptHistory::with_limit(50);
        let statistics = StatisticsAggregator::new();
        let tracking = TrackingSystem::with_config(statistics, history);

        pretty_assertions::assert_eq!(tracking.history.max_attempts, 50);
    }

    #[test]
    fn test_tracking_system_record_outcome() {
        let mut tracking = TrackingSystem::new();

        let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
        tracking.record_outcome(&outcome);

        // Statistics should be updated even without a RouteAttempt
        assert!(tracking.get_statistics("route-1").is_some());
        // But history should still be empty (record_outcome only updates statistics)
        assert!(tracking.history.is_empty());
    }

    #[test]
    fn test_tracking_system_reset_all() {
        let mut tracking = TrackingSystem::new();

        // Record via attempt (updates both statistics and history)
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

        // Record outcome only (updates statistics only)
        let outcome2 = ExecutionOutcome::success("route-2".to_string(), 150.0, 20, 10, 200);
        tracking.record_outcome(&outcome2);

        assert!(tracking.get_statistics("route-1").is_some());
        assert!(tracking.get_statistics("route-2").is_some());
        pretty_assertions::assert_eq!(tracking.history.len(), 1);

        tracking.reset_all();

        // Everything should be cleared
        assert!(tracking.get_statistics("route-1").is_none());
        assert!(tracking.get_statistics("route-2").is_none());
        assert!(tracking.history.is_empty());
    }

    #[test]
    fn test_tracking_system_get_attempt_metrics_none_for_unknown_route() {
        let tracking = TrackingSystem::new();
        assert!(tracking.get_attempt_metrics("nonexistent").is_none());
    }

    #[test]
    fn test_tracking_system_get_attempts_empty_for_unknown_route() {
        let tracking = TrackingSystem::new();
        assert!(tracking.get_attempts("nonexistent").is_empty());
    }

    // --- Fallback effective_route integration ---

    #[test]
    fn test_fallback_attempts_filter_by_effective_route() {
        let mut history = AttemptHistory::new();

        // A failure with fallback: route_id=fallback, effective_route=original
        let decision_context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["fallback-route".to_string()],
            SelectionMode::Fallback,
            "fallback-route".to_string(),
        );
        let outcome = ExecutionOutcome::failure(
            "fallback-route".to_string(),
            300.0,
            500,
            true,
            Some("original-route".to_string()),
        );
        let attempt = RouteAttempt::new("req-1".to_string(), decision_context, outcome);
        history.record(attempt);

        // Should find the attempt via the original (effective) route
        let by_original = history.get_attempts_for_route("original-route");
        pretty_assertions::assert_eq!(by_original.len(), 1);

        // Should NOT find it via the fallback route_id
        let by_fallback = history.get_attempts_for_route("fallback-route");
        assert!(by_fallback.is_empty());
    }

    // --- All SelectionMode variants ---

    #[rstest::rstest]
    #[case(SelectionMode::Weighted)]
    #[case(SelectionMode::Thompson)]
    #[case(SelectionMode::TimeAware)]
    #[case(SelectionMode::QuotaAware)]
    #[case(SelectionMode::Adaptive)]
    #[case(SelectionMode::Fallback)]
    #[case(SelectionMode::Manual)]
    fn test_all_selection_mode_variants(#[case] mode: SelectionMode) {
        let mut history = AttemptHistory::new();

        let decision_context = DecisionContext::new(
            "req-1".to_string(),
            "model-1".to_string(),
            vec!["route-1".to_string()],
            mode.clone(),
            "route-1".to_string(),
        );
        let outcome = ExecutionOutcome::success("route-1".to_string(), 100.0, 10, 5, 200);
        let attempt = RouteAttempt::new("req-1".to_string(), decision_context, outcome);
        history.record(attempt);

        let results = history.get_attempts_by_selection_mode(&mode);
        pretty_assertions::assert_eq!(results.len(), 1);
    }
}
