use crate::trace::TraceSpan;
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Trait for collecting and storing traces
#[async_trait]
pub trait TraceCollector: Send + Sync {
    /// Record a trace span
    async fn record_trace(&self, trace: TraceSpan);

    /// Retrieve all recorded traces
    async fn get_traces(&self) -> Vec<TraceSpan>;

    /// Get the number of traces currently stored
    async fn trace_count(&self) -> usize;
}

/// In-memory trace collector with a bounded buffer
#[derive(Clone)]
pub struct MemoryTraceCollector {
    traces: Arc<RwLock<VecDeque<TraceSpan>>>,
    max_size: usize,
}

impl MemoryTraceCollector {
    /// Create a new memory trace collector with a maximum buffer size.
    ///
    /// When the buffer is full, the oldest trace is evicted.
    ///
    /// # Examples
    ///
    /// ```
    /// # use llm_tracing::{MemoryTraceCollector, TraceCollector, TraceSpan};
    /// # #[tokio::main]
    /// # async fn main() {
    /// let collector = MemoryTraceCollector::new(10);
    ///
    /// let trace = TraceSpan::new(
    ///     "req-1".to_string(),
    ///     "openai".to_string(),
    ///     "gpt-4".to_string(),
    ///     None,
    /// );
    ///
    /// collector.record_trace(trace).await;
    /// assert_eq!(collector.trace_count().await, 1);
    /// # }
    /// ```
    pub fn new(max_size: usize) -> Self {
        Self {
            traces: Arc::new(RwLock::new(VecDeque::with_capacity(max_size))),
            max_size,
        }
    }

    /// Create a collector with default buffer size (1000 traces)
    pub fn with_default_size() -> Self {
        Self::new(1000)
    }

    /// Clear all traces
    pub async fn clear(&self) {
        let mut traces = self.traces.write().await;
        traces.clear();
    }
}

#[async_trait]
impl TraceCollector for MemoryTraceCollector {
    async fn record_trace(&self, trace: TraceSpan) {
        if self.max_size == 0 {
            return;
        }
        let mut traces = self.traces.write().await;

        // Add the new trace
        if traces.len() >= self.max_size {
            // Remove oldest trace if buffer is full
            traces.pop_front();
        }
        traces.push_back(trace);
    }

    async fn get_traces(&self) -> Vec<TraceSpan> {
        let traces = self.traces.read().await;
        traces.iter().cloned().collect()
    }

    async fn trace_count(&self) -> usize {
        let traces = self.traces.read().await;
        traces.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::TraceSpan;

    #[tokio::test]
    async fn test_memory_collector_record() {
        let collector = MemoryTraceCollector::new(10);

        let trace = TraceSpan::new(
            "req-1".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );

        collector.record_trace(trace).await;

        assert_eq!(collector.trace_count().await, 1);

        let traces = collector.get_traces().await;
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0].request_id, "req-1");
    }

    #[tokio::test]
    async fn test_memory_collector_bounded() {
        let collector = MemoryTraceCollector::new(3);

        // Add 5 traces
        for i in 1..=5 {
            let trace = TraceSpan::new(
                format!("req-{i}"),
                "openai".to_string(),
                "gpt-4".to_string(),
                None,
            );
            collector.record_trace(trace).await;
        }

        // Should only keep the last 3
        assert_eq!(collector.trace_count().await, 3);

        let traces = collector.get_traces().await;
        assert_eq!(traces[0].request_id, "req-3");
        assert_eq!(traces[1].request_id, "req-4");
        assert_eq!(traces[2].request_id, "req-5");
    }

    #[tokio::test]
    async fn test_memory_collector_clear() {
        let collector = MemoryTraceCollector::new(10);

        let trace = TraceSpan::new(
            "req-1".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );

        collector.record_trace(trace).await;
        assert_eq!(collector.trace_count().await, 1);

        collector.clear().await;
        assert_eq!(collector.trace_count().await, 0);
    }

    #[tokio::test]
    async fn test_collector_concurrent() {
        let collector = Arc::new(MemoryTraceCollector::new(100));
        let mut handles = vec![];

        // Spawn 10 concurrent tasks recording traces
        for i in 0..10 {
            let collector = Arc::clone(&collector);
            let handle = tokio::spawn(async move {
                let trace = TraceSpan::new(
                    format!("req-{i}"),
                    "openai".to_string(),
                    "gpt-4".to_string(),
                    None,
                );
                collector.record_trace(trace).await;
            });
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.expect("Internal logic invariant should hold");
        }

        assert_eq!(collector.trace_count().await, 10);
    }

    // ===== Edge Case Tests =====

    #[tokio::test]
    async fn test_memory_collector_zero_size() {
        // Zero size should not store any traces
        let collector = MemoryTraceCollector::new(0);

        let trace = TraceSpan::new(
            "req-1".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );

        collector.record_trace(trace).await;

        assert_eq!(collector.trace_count().await, 0);
    }

    #[tokio::test]
    async fn test_memory_collector_max_size() {
        // Test with large max size
        let collector = MemoryTraceCollector::new(10000);

        for i in 0..100 {
            let trace = TraceSpan::new(
                format!("req-{i}"),
                "openai".to_string(),
                "gpt-4".to_string(),
                None,
            );
            collector.record_trace(trace).await;
        }

        assert_eq!(collector.trace_count().await, 100);
    }

    #[tokio::test]
    async fn test_record_trace_with_none_fields() {
        let collector = MemoryTraceCollector::new(10);

        // Create trace with None fields
        let trace = TraceSpan::new(
            "req-none-fields".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None, // auth_id is None
        );
        // All optional fields remain None by default
        assert!(trace.auth_id.is_none());
        assert!(trace.input_tokens.is_none());
        assert!(trace.output_tokens.is_none());
        assert!(trace.prompt.is_none());
        assert!(trace.error_message.is_none());

        collector.record_trace(trace).await;

        let traces = collector.get_traces().await;
        assert_eq!(traces.len(), 1);
        assert!(traces[0].auth_id.is_none());
    }

    #[tokio::test]
    async fn test_collector_stress_test() {
        let collector = Arc::new(MemoryTraceCollector::new(1000));
        let mut handles = vec![];

        // Spawn 100 concurrent tasks, each recording 10 traces
        for task_id in 0..100 {
            let collector = Arc::clone(&collector);
            let handle = tokio::spawn(async move {
                for i in 0..10 {
                    let trace = TraceSpan::new(
                        format!("task-{task_id}-req-{i}"),
                        "openai".to_string(),
                        "gpt-4".to_string(),
                        Some(format!("user-{task_id}")),
                    );
                    collector.record_trace(trace).await;
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.expect("Internal logic invariant should hold");
        }

        // Should have 1000 traces (or capped at max_size)
        assert_eq!(collector.trace_count().await, 1000);
    }

    #[tokio::test]
    async fn test_size_limit_enforcement() {
        let collector = MemoryTraceCollector::new(5);

        // Add 10 traces to a buffer of size 5
        for i in 0..10 {
            let trace = TraceSpan::new(
                format!("req-{i}"),
                "openai".to_string(),
                "gpt-4".to_string(),
                None,
            );
            collector.record_trace(trace).await;
        }

        // Should only keep the last 5
        assert_eq!(collector.trace_count().await, 5);

        let traces = collector.get_traces().await;
        // Verify oldest traces were evicted
        assert_eq!(traces[0].request_id, "req-5");
        assert_eq!(traces[4].request_id, "req-9");
    }

    #[tokio::test]
    async fn test_collector_clone_independence() {
        let collector1 = MemoryTraceCollector::new(10);

        // Clone shares the same internal storage due to Arc
        let collector2 = collector1.clone();

        let trace = TraceSpan::new(
            "req-shared".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            None,
        );
        collector1.record_trace(trace).await;

        // Both collectors should see the trace (shared state)
        assert_eq!(collector1.trace_count().await, 1);
        assert_eq!(collector2.trace_count().await, 1);
    }
}
