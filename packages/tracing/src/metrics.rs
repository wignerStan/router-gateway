use crate::trace::TraceSpan;
use std::collections::HashMap;

/// Aggregated metrics for traces
#[derive(Debug, Clone, Default)]
pub struct TraceMetrics {
    /// Total number of requests
    pub total_requests: u64,
    /// Number of successful requests (2xx status codes)
    pub successful_requests: u64,
    /// Number of failed requests (non-2xx status codes)
    pub failed_requests: u64,
    /// Number of requests with recorded latency
    pub latency_count: u64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// EWMA (Exponentially Weighted Moving Average) of latency
    pub ewma_latency_ms: f64,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,

    /// Per-provider metrics
    pub provider_metrics: HashMap<String, ProviderMetrics>,

    /// Per-model metrics
    pub model_metrics: HashMap<String, ModelMetrics>,
}

/// Metrics aggregated per provider
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ProviderMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub latency_count: u64,
    pub avg_latency_ms: f64,
    pub ewma_latency_ms: f64,
}

/// Metrics aggregated per model
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ModelMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub latency_count: u64,
    pub avg_latency_ms: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}

impl TraceMetrics {
    /// Create new empty metrics
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate EWMA with a smoothing factor (alpha)
    /// alpha = 2.0 / (N + 1) where N is the period
    /// For N=20 (commonly used), alpha ≈ 0.095
    fn calculate_ewma(prev_ewma: f64, new_value: f64, alpha: f64) -> f64 {
        if prev_ewma == 0.0 {
            new_value
        } else {
            alpha * new_value + (1.0 - alpha) * prev_ewma
        }
    }

    /// Update metrics with a new trace
    pub fn update(&mut self, trace: &TraceSpan) {
        self.total_requests += 1;

        let is_success = trace.is_success();
        if is_success {
            self.successful_requests += 1;
        } else {
            self.failed_requests += 1;
        }

        // Update latency metrics
        if let Some(latency) = trace.latency_ms {
            self.latency_count += 1;
            let latency_f64 = latency as f64;

            // Simple average
            self.avg_latency_ms = (self.avg_latency_ms * (self.latency_count - 1) as f64
                + latency_f64)
                / self.latency_count as f64;

            // EWMA with alpha=0.1 (smoothing factor)
            self.ewma_latency_ms = Self::calculate_ewma(self.ewma_latency_ms, latency_f64, 0.1);
        }

        // Update success rate
        self.success_rate = self.successful_requests as f64 / self.total_requests as f64;

        // Update per-provider metrics
        self.provider_metrics
            .entry(trace.provider.clone())
            .or_default()
            .update(trace);

        // Update per-model metrics
        self.model_metrics
            .entry(trace.model.clone())
            .or_default()
            .update(trace);
    }

    /// Aggregate metrics from a collection of traces
    pub fn aggregate(traces: &[TraceSpan]) -> Self {
        let mut metrics = Self::new();
        for trace in traces {
            metrics.update(trace);
        }
        metrics
    }

    /// Get percentile approximation using simple interpolation
    /// Note: This requires storing all values; for production, consider t-digest or similar
    pub fn get_percentile(&self, _percentile: f64) -> Option<f64> {
        // Placeholder: would need to store actual latency values
        // For production, use a proper percentile approximation algorithm
        None
    }
}

impl ProviderMetrics {
    fn update(&mut self, trace: &TraceSpan) {
        self.total_requests += 1;
        if trace.is_success() {
            self.successful_requests += 1;
        }

        if let Some(latency) = trace.latency_ms {
            self.latency_count += 1;
            let latency_f64 = latency as f64;
            self.avg_latency_ms = (self.avg_latency_ms * (self.latency_count - 1) as f64
                + latency_f64)
                / self.latency_count as f64;
            self.ewma_latency_ms =
                TraceMetrics::calculate_ewma(self.ewma_latency_ms, latency_f64, 0.1);
        }
    }

    /// Get success rate for this provider
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.successful_requests as f64 / self.total_requests as f64
        }
    }
}

impl ModelMetrics {
    fn update(&mut self, trace: &TraceSpan) {
        self.total_requests += 1;
        if trace.is_success() {
            self.successful_requests += 1;
        }

        if let Some(latency) = trace.latency_ms {
            self.latency_count += 1;
            let latency_f64 = latency as f64;
            self.avg_latency_ms = (self.avg_latency_ms * (self.latency_count - 1) as f64
                + latency_f64)
                / self.latency_count as f64;
        }

        if let Some(tokens) = trace.input_tokens {
            self.total_input_tokens += tokens as u64;
        }

        if let Some(tokens) = trace.output_tokens {
            self.total_output_tokens += tokens as u64;
        }
    }

    /// Get success rate for this model
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.successful_requests as f64 / self.total_requests as f64
        }
    }

    /// Get average tokens per request (input + output)
    pub fn avg_total_tokens(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            (self.total_input_tokens + self.total_output_tokens) as f64 / self.total_requests as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::TraceSpan;
    use chrono::Utc;

    fn create_test_trace(
        request_id: &str,
        provider: &str,
        model: &str,
        latency_ms: u64,
        status_code: u16,
    ) -> TraceSpan {
        let mut trace = TraceSpan::new(
            request_id.to_string(),
            provider.to_string(),
            model.to_string(),
            None,
        );

        trace.start_time = Utc::now() - chrono::Duration::milliseconds(latency_ms as i64);
        trace.complete(status_code);

        trace
    }

    #[test]
    fn test_metrics_aggregation() {
        let traces = vec![
            create_test_trace("req-1", "openai", "gpt-4", 100, 200),
            create_test_trace("req-2", "openai", "gpt-4", 200, 200),
            create_test_trace("req-3", "anthropic", "claude-3", 150, 200),
            create_test_trace("req-4", "google", "gemini-pro", 300, 500), // Failed
        ];

        let metrics = TraceMetrics::aggregate(&traces);

        assert_eq!(metrics.total_requests, 4);
        assert_eq!(metrics.successful_requests, 3);
        assert_eq!(metrics.failed_requests, 1);
        assert!((metrics.success_rate - 0.75).abs() < 0.01);
        assert!((metrics.avg_latency_ms - 187.5).abs() < 0.1);
    }

    #[test]
    fn test_provider_metrics() {
        let traces = vec![
            create_test_trace("req-1", "openai", "gpt-4", 100, 200),
            create_test_trace("req-2", "openai", "gpt-3.5", 200, 200),
            create_test_trace("req-3", "anthropic", "claude-3", 150, 500),
        ];

        let metrics = TraceMetrics::aggregate(&traces);

        // Check OpenAI provider metrics
        let openai_metrics = metrics
            .provider_metrics
            .get("openai")
            .expect("value must be present");
        assert_eq!(openai_metrics.total_requests, 2);
        assert_eq!(openai_metrics.successful_requests, 2);
        assert!((openai_metrics.avg_latency_ms - 150.0).abs() < 0.1);

        // Check Anthropic provider metrics
        let anthropic_metrics = metrics
            .provider_metrics
            .get("anthropic")
            .expect("value must be present");
        assert_eq!(anthropic_metrics.total_requests, 1);
        assert_eq!(anthropic_metrics.successful_requests, 0);
    }

    #[test]
    fn test_model_metrics() {
        let mut trace1 = create_test_trace("req-1", "openai", "gpt-4", 100, 200);
        trace1.input_tokens = Some(100);
        trace1.output_tokens = Some(50);

        let mut trace2 = create_test_trace("req-2", "openai", "gpt-4", 200, 200);
        trace2.input_tokens = Some(200);
        trace2.output_tokens = Some(100);

        let metrics = TraceMetrics::aggregate(&[trace1, trace2]);

        let gpt4_metrics = metrics
            .model_metrics
            .get("gpt-4")
            .expect("value must be present");
        assert_eq!(gpt4_metrics.total_requests, 2);
        assert_eq!(gpt4_metrics.total_input_tokens, 300);
        assert_eq!(gpt4_metrics.total_output_tokens, 150);
        assert!((gpt4_metrics.avg_total_tokens() - 225.0).abs() < 0.1);
    }

    #[test]
    fn test_ewma_calculation() {
        let ewma = TraceMetrics::calculate_ewma(0.0, 100.0, 0.1);
        assert_eq!(ewma, 100.0);

        let ewma = TraceMetrics::calculate_ewma(100.0, 200.0, 0.1);
        assert!((ewma - 110.0).abs() < 0.01);

        let ewma = TraceMetrics::calculate_ewma(110.0, 150.0, 0.1);
        assert!((ewma - 114.0).abs() < 0.01);
    }

    #[test]
    fn test_metrics_incremental() {
        let mut metrics = TraceMetrics::new();

        metrics.update(&create_test_trace("req-1", "openai", "gpt-4", 100, 200));
        assert_eq!(metrics.total_requests, 1);
        assert_eq!(metrics.successful_requests, 1);

        metrics.update(&create_test_trace("req-2", "openai", "gpt-4", 200, 500));
        assert_eq!(metrics.total_requests, 2);
        assert_eq!(metrics.successful_requests, 1);
        assert_eq!(metrics.failed_requests, 1);
        assert!((metrics.success_rate - 0.5).abs() < 0.01);
    }

    // ===== Edge Case Tests =====

    #[test]
    fn test_update_none_latency() {
        let mut metrics = TraceMetrics::new();

        // Create a trace without completing it (no latency)
        let trace = TraceSpan::new(
            "req-no-latency".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );

        metrics.update(&trace);

        assert_eq!(metrics.total_requests, 1);
        assert_eq!(metrics.avg_latency_ms, 0.0); // No latency recorded
        assert_eq!(metrics.ewma_latency_ms, 0.0);
    }

    #[test]
    fn test_update_very_large_latency() {
        let mut metrics = TraceMetrics::new();

        // Create a trace with a large but safe latency value
        let mut trace = TraceSpan::new(
            "req-large-latency".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );
        // Use a reasonable large value that won't overflow chrono
        trace.start_time = Utc::now() - chrono::Duration::seconds(86400); // 1 day
        trace.complete(200);

        // The latency should be computed
        metrics.update(&trace);
        assert_eq!(metrics.total_requests, 1);
        assert!(metrics.avg_latency_ms > 0.0);
    }

    #[test]
    fn test_aggregate_empty_array() {
        let traces: Vec<TraceSpan> = vec![];
        let metrics = TraceMetrics::aggregate(&traces);

        assert_eq!(metrics.total_requests, 0);
        assert_eq!(metrics.successful_requests, 0);
        assert_eq!(metrics.failed_requests, 0);
        assert!(metrics.provider_metrics.is_empty());
        assert!(metrics.model_metrics.is_empty());
    }

    #[test]
    fn test_aggregate_large_batch() {
        // Create 1000 traces
        let traces: Vec<TraceSpan> = (0..1000)
            .map(|i| {
                let status = if i % 4 == 0 { 500 } else { 200 }; // 25% failure rate
                create_test_trace(
                    &format!("req-{}", i),
                    if i % 2 == 0 { "openai" } else { "anthropic" },
                    if i % 3 == 0 { "gpt-4" } else { "gpt-3.5" },
                    100 + (i % 100) as u64,
                    status,
                )
            })
            .collect();

        let metrics = TraceMetrics::aggregate(&traces);

        assert_eq!(metrics.total_requests, 1000);
        assert_eq!(metrics.successful_requests, 750); // 75% success
        assert_eq!(metrics.failed_requests, 250); // 25% failure
        assert!((metrics.success_rate - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_get_percentile_returns_none() {
        let metrics = TraceMetrics::new();

        // Current implementation returns None (placeholder)
        assert_eq!(metrics.get_percentile(0.0), None);
        assert_eq!(metrics.get_percentile(50.0), None);
        assert_eq!(metrics.get_percentile(90.0), None);
        assert_eq!(metrics.get_percentile(95.0), None);
        assert_eq!(metrics.get_percentile(99.0), None);
        assert_eq!(metrics.get_percentile(100.0), None);
    }

    #[test]
    fn test_provider_metrics_none_latency() {
        let trace = TraceSpan::new(
            "req-no-lat".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );
        // Don't complete - no latency

        let mut provider_metrics = ProviderMetrics::default();
        provider_metrics.update(&trace);

        assert_eq!(provider_metrics.total_requests, 1);
        // When latency is None, the metrics should not be updated from 0.0
        assert_eq!(provider_metrics.avg_latency_ms, 0.0);
        assert_eq!(provider_metrics.ewma_latency_ms, 0.0);
    }

    #[test]
    fn test_provider_metrics_zero_latency() {
        // Create trace with zero latency (start == end)
        let mut trace = TraceSpan::new(
            "req-zero-lat".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );
        trace.complete(200);
        // Complete immediately - latency should be 0 or very close to it

        let mut provider_metrics = ProviderMetrics::default();
        provider_metrics.update(&trace);

        assert_eq!(provider_metrics.total_requests, 1);
        assert!(provider_metrics.avg_latency_ms < 10.0); // Should be very small
    }

    #[test]
    fn test_model_metrics_avg_total_tokens_zero_requests() {
        let model_metrics = ModelMetrics::default();

        assert_eq!(model_metrics.total_requests, 0);
        assert_eq!(model_metrics.avg_total_tokens(), 0.0);
    }

    #[test]
    fn test_model_metrics_avg_total_tokens_with_data() {
        let model_metrics = ModelMetrics {
            total_requests: 10,
            total_input_tokens: 1000,
            total_output_tokens: 500,
            ..Default::default()
        };

        assert!((model_metrics.avg_total_tokens() - 150.0).abs() < 0.01);
    }

    #[test]
    fn test_model_metrics_no_overflow() {
        let model_metrics = ModelMetrics {
            total_requests: 1,
            total_input_tokens: u32::MAX as u64,
            total_output_tokens: u32::MAX as u64,
            ..Default::default()
        };

        // Should not panic and produce a valid result
        let avg = model_metrics.avg_total_tokens();
        assert!(avg > 0.0);
        assert!(avg.is_finite());
    }

    #[test]
    fn test_provider_success_rate() {
        let mut provider = ProviderMetrics::default();
        assert_eq!(provider.success_rate(), 0.0);

        provider.total_requests = 10;
        provider.successful_requests = 7;
        assert!((provider.success_rate() - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_model_success_rate() {
        let mut model = ModelMetrics::default();
        assert_eq!(model.success_rate(), 0.0);

        model.total_requests = 4;
        model.successful_requests = 3;
        assert!((model.success_rate() - 0.75).abs() < 0.01);
    }
}
