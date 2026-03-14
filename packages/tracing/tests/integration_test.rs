//! Integration tests for the llm-tracing package
//!
//! These tests verify end-to-end functionality across multiple components.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use chrono::Utc;
use llm_tracing::{
    MemoryTraceCollector, TraceCollector, TraceMetrics, TraceSpan, TracingMiddleware,
    TracingMiddlewareBuilder,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;
use tower::util::ServiceExt; // Required for oneshot method

// Helper to create a completed trace span
fn create_completed_trace(
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
        Some("test-user".to_string()),
    );
    trace.start_time = Utc::now() - chrono::Duration::milliseconds(latency_ms as i64);
    trace.input_tokens = Some(100);
    trace.output_tokens = Some(50);
    trace.complete(status_code);
    trace
}

// ===== End-to-End HTTP Request Tests =====

#[tokio::test]
async fn test_end_to_end_http_request_tracing() {
    // Create collector and middleware
    let collector: Arc<dyn TraceCollector> = Arc::new(MemoryTraceCollector::new(100));
    let middleware = TracingMiddleware::new(Arc::clone(&collector));

    // Create a simple test handler
    async fn test_handler() -> impl IntoResponse {
        (StatusCode::OK, "Hello, World!")
    }

    // Build router with tracing middleware
    let app = Router::new().route("/test", get(test_handler)).layer(
        axum::middleware::from_fn_with_state(middleware.clone(), llm_tracing::tracing_middleware),
    );

    // Make a test request
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/test")
                .header("x-request-id", "test-req-123")
                .header("x-llm-provider", "test-provider")
                .header("x-llm-model", "test-model")
                .body(Body::empty())
                .expect("Tracing integration test should succeed"),
        )
        .await
        .expect("Tracing integration test should succeed");

    // Verify response
    assert_eq!(response.status(), StatusCode::OK);

    // Verify trace was recorded
    let traces = collector.get_traces().await;
    assert_eq!(traces.len(), 1);

    let trace = &traces[0];
    assert_eq!(trace.request_id, "test-req-123");
    assert_eq!(trace.provider, "test-provider");
    assert_eq!(trace.model, "test-model");
    assert_eq!(trace.status_code, Some(200));
    assert!(trace.latency_ms.is_some());
    assert!(trace.is_success());
}

#[tokio::test]
async fn test_end_to_end_failed_request() {
    let collector: Arc<dyn TraceCollector> = Arc::new(MemoryTraceCollector::new(100));
    let middleware = TracingMiddleware::new(Arc::clone(&collector));

    // Create a handler that returns an error
    async fn error_handler() -> impl IntoResponse {
        (StatusCode::INTERNAL_SERVER_ERROR, "Error!")
    }

    let app = Router::new().route("/error", get(error_handler)).layer(
        axum::middleware::from_fn_with_state(middleware.clone(), llm_tracing::tracing_middleware),
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/error")
                .header("x-request-id", "error-req")
                .body(Body::empty())
                .expect("Tracing integration test should succeed"),
        )
        .await
        .expect("Tracing integration test should succeed");

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let traces = collector.get_traces().await;
    assert_eq!(traces.len(), 1);

    let trace = &traces[0];
    assert_eq!(trace.request_id, "error-req");
    assert_eq!(trace.status_code, Some(500));
    assert!(!trace.is_success());
}

// ===== Concurrent Access Tests =====

#[tokio::test]
async fn test_concurrent_collector_access() {
    let collector = Arc::new(MemoryTraceCollector::new(1000));
    let mut tasks = JoinSet::new();

    // Spawn multiple concurrent tasks
    for i in 0..50 {
        let collector = Arc::clone(&collector);
        tasks.spawn(async move {
            for j in 0..20 {
                let trace = create_completed_trace(
                    &format!("concurrent-{i}-{j}"),
                    "openai",
                    "gpt-4",
                    100,
                    200,
                );
                collector.record_trace(trace).await;
            }
        });
    }

    // Wait for all tasks to complete
    while tasks.join_next().await.is_some() {}

    // Verify all traces were recorded
    assert_eq!(collector.trace_count().await, 1000); // Capped at max_size
}

#[tokio::test]
async fn test_concurrent_read_write() {
    let collector = Arc::new(MemoryTraceCollector::new(500));
    let mut tasks = JoinSet::new();

    // Spawn writers
    for i in 0..25 {
        let collector = Arc::clone(&collector);
        tasks.spawn(async move {
            for j in 0..40 {
                let trace = create_completed_trace(
                    &format!("writer-{i}-{j}"),
                    "anthropic",
                    "claude-3",
                    150,
                    200,
                );
                collector.record_trace(trace).await;
                tokio::time::sleep(Duration::from_micros(10)).await;
            }
        });
    }

    // Spawn readers
    for _ in 0..25 {
        let collector = Arc::clone(&collector);
        tasks.spawn(async move {
            for _ in 0..40 {
                let count = collector.trace_count().await;
                assert!(count <= 500);
                let _traces = collector.get_traces().await;
                tokio::time::sleep(Duration::from_micros(10)).await;
            }
        });
    }

    // Wait for all tasks
    while tasks.join_next().await.is_some() {}

    // Final state should be consistent
    let final_count = collector.trace_count().await;
    assert!(final_count <= 500);
    let traces = collector.get_traces().await;
    assert_eq!(traces.len(), final_count);
}

// ===== Error Propagation Tests =====

#[tokio::test]
async fn test_error_trace_creation() {
    let mut trace = TraceSpan::new(
        "error-test".to_string(),
        "openai".to_string(),
        "gpt-4".to_string(),
        None,
    );

    trace.set_error("Rate limit exceeded".to_string());

    assert!(trace.error_message.is_some());
    assert_eq!(trace.status_code, Some(500));
    assert!(!trace.is_success());
    assert!(trace.end_time.is_some());
}

#[tokio::test]
async fn test_error_trace_to_metrics() {
    let mut metrics = TraceMetrics::new();

    // Create a trace with error
    let mut trace = TraceSpan::new(
        "error-metrics".to_string(),
        "openai".to_string(),
        "gpt-4".to_string(),
        None,
    );
    trace.set_error("Internal error".to_string());

    metrics.update(&trace);

    assert_eq!(metrics.total_requests, 1);
    assert_eq!(metrics.failed_requests, 1);
    assert_eq!(metrics.successful_requests, 0);

    // Provider metrics should also reflect the error
    let provider = metrics
        .provider_metrics
        .get("openai")
        .expect("Tracing integration test should succeed");
    assert_eq!(provider.successful_requests, 0);
    assert_eq!(provider.total_requests, 1);
}

// ===== Memory Management Tests =====

#[tokio::test]
async fn test_collector_eviction_policy() {
    let collector = MemoryTraceCollector::new(10);

    // Add 20 traces to a collector with capacity 10
    for i in 0..20 {
        let trace = create_completed_trace(&format!("req-{i}"), "openai", "gpt-4", 100, 200);
        collector.record_trace(trace).await;
    }

    // Should only have the last 10
    assert_eq!(collector.trace_count().await, 10);

    let traces = collector.get_traces().await;
    // First trace should be req-10 (oldest of the remaining)
    assert_eq!(traces[0].request_id, "req-10");
    // Last trace should be req-19
    assert_eq!(traces[9].request_id, "req-19");
}

#[tokio::test]
async fn test_collector_clear_and_refill() {
    let collector = MemoryTraceCollector::new(100);

    // Fill with traces
    for i in 0..50 {
        let trace = create_completed_trace(&format!("req-{i}"), "openai", "gpt-4", 100, 200);
        collector.record_trace(trace).await;
    }
    assert_eq!(collector.trace_count().await, 50);

    // Clear
    collector.clear().await;
    assert_eq!(collector.trace_count().await, 0);

    // Refill
    for i in 0..30 {
        let trace =
            create_completed_trace(&format!("new-req-{i}"), "anthropic", "claude-3", 150, 200);
        collector.record_trace(trace).await;
    }
    assert_eq!(collector.trace_count().await, 30);

    let traces = collector.get_traces().await;
    assert!(traces.iter().all(|t| t.request_id.starts_with("new-req")));
}

// ===== Collector to Metrics Integration =====

#[tokio::test]
async fn test_collector_to_metrics_integration() {
    let collector = MemoryTraceCollector::new(100);

    // Record various traces
    let traces_to_add = vec![
        create_completed_trace("req-1", "openai", "gpt-4", 100, 200),
        create_completed_trace("req-2", "openai", "gpt-4", 150, 200),
        create_completed_trace("req-3", "openai", "gpt-3.5", 50, 200),
        create_completed_trace("req-4", "anthropic", "claude-3", 200, 500),
        create_completed_trace("req-5", "anthropic", "claude-3", 250, 200),
        create_completed_trace("req-6", "google", "gemini-pro", 80, 429),
    ];

    for trace in traces_to_add {
        collector.record_trace(trace).await;
    }

    // Get traces and compute metrics
    let traces = collector.get_traces().await;
    let metrics = TraceMetrics::aggregate(&traces);

    // Verify aggregated metrics
    assert_eq!(metrics.total_requests, 6);
    assert_eq!(metrics.successful_requests, 4);
    assert_eq!(metrics.failed_requests, 2);

    // Verify provider breakdown
    assert_eq!(metrics.provider_metrics.len(), 3);

    let openai = metrics
        .provider_metrics
        .get("openai")
        .expect("Tracing integration test should succeed");
    assert_eq!(openai.total_requests, 3);
    assert_eq!(openai.successful_requests, 3);

    let anthropic = metrics
        .provider_metrics
        .get("anthropic")
        .expect("Tracing integration test should succeed");
    assert_eq!(anthropic.total_requests, 2);
    assert_eq!(anthropic.successful_requests, 1);

    let google = metrics
        .provider_metrics
        .get("google")
        .expect("Tracing integration test should succeed");
    assert_eq!(google.total_requests, 1);
    assert_eq!(google.successful_requests, 0);

    // Verify model breakdown
    assert_eq!(metrics.model_metrics.len(), 4);
}

// ===== Builder Pattern Tests =====

#[tokio::test]
async fn test_middleware_builder_integration() {
    let collector = Arc::new(MemoryTraceCollector::new(50)) as Arc<dyn TraceCollector>;

    let middleware = TracingMiddlewareBuilder::new()
        .with_collector(Arc::clone(&collector))
        .build();

    // Verify middleware was created correctly
    async fn test_handler() -> impl IntoResponse {
        StatusCode::OK
    }

    let app = Router::new().route("/test", get(test_handler)).layer(
        axum::middleware::from_fn_with_state(middleware.clone(), llm_tracing::tracing_middleware),
    );

    // Make request
    let _response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/test")
                .body(Body::empty())
                .expect("Tracing integration test should succeed"),
        )
        .await
        .expect("Tracing integration test should succeed");

    // Verify collector received the trace
    assert_eq!(collector.trace_count().await, 1);
}

// ===== Stress Test =====

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_high_throughput_tracing() {
    let collector = Arc::new(MemoryTraceCollector::new(10000));
    let mut tasks = JoinSet::new();

    // Simulate high throughput: 100 tasks, each recording 100 traces
    for task_id in 0..100 {
        let collector = Arc::clone(&collector);
        tasks.spawn(async move {
            for i in 0..100 {
                let trace = create_completed_trace(
                    &format!("high-throughput-{task_id}-{i}"),
                    "openai",
                    "gpt-4",
                    50 + (i % 50) as u64,
                    if i % 10 == 0 { 500 } else { 200 },
                );
                collector.record_trace(trace).await;
            }
        });
    }

    // Wait for completion
    while tasks.join_next().await.is_some() {}

    // Should be at capacity
    assert_eq!(collector.trace_count().await, 10000);

    // Compute metrics from collected traces
    let traces = collector.get_traces().await;
    let metrics = TraceMetrics::aggregate(&traces);

    assert_eq!(metrics.total_requests, 10000);
    // 10% failure rate (i % 10 == 0)
    assert!((metrics.success_rate - 0.9).abs() < 0.05);
}
