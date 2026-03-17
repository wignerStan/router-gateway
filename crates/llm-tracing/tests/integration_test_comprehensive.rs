#![allow(
    // ALLOW: Arc::clone in test setup is idiomatic — readability outweighs ref-count cost.
    clippy::clone_on_ref_ptr,
    // ALLOW: Test helpers are defined after test functions for readability — statements after items is fine.
    clippy::items_after_statements,
    // ALLOW: Intentional exact float comparisons in test assertions (e.g., assert_eq!(rate, 1.0)).
    clippy::float_cmp,
)]
//! Expanded integration tests for the llm-tracing package
//!
//! These tests provide comprehensive coverage of all public types and methods
//! across the tracing, collector, metrics, and middleware components.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::{get, post},
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
use tower::util::ServiceExt;

// ===== Helpers =====

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

fn create_trace_with_tokens(
    request_id: &str,
    provider: &str,
    model: &str,
    input_tokens: u32,
    output_tokens: u32,
    status_code: u16,
) -> TraceSpan {
    let mut trace = TraceSpan::new(
        request_id.to_string(),
        provider.to_string(),
        model.to_string(),
        None,
    );
    trace.input_tokens = Some(input_tokens);
    trace.output_tokens = Some(output_tokens);
    trace.complete(status_code);
    trace
}

fn make_test_app(middleware: TracingMiddleware) -> Router {
    async fn ok_handler() -> impl IntoResponse {
        (StatusCode::OK, "OK")
    }

    async fn post_handler() -> impl IntoResponse {
        (StatusCode::CREATED, "Created")
    }

    async fn error_handler() -> impl IntoResponse {
        (StatusCode::INTERNAL_SERVER_ERROR, "Error")
    }

    async fn not_found_handler() -> impl IntoResponse {
        (StatusCode::NOT_FOUND, "Not Found")
    }

    async fn rate_limit_handler() -> impl IntoResponse {
        (StatusCode::TOO_MANY_REQUESTS, "Rate Limited")
    }

    Router::new()
        .route("/ok", get(ok_handler))
        .route("/created", post(post_handler))
        .route("/error", get(error_handler))
        .route("/not-found", get(not_found_handler))
        .route("/rate-limit", get(rate_limit_handler))
        .layer(axum::middleware::from_fn_with_state(
            middleware,
            llm_tracing::tracing_middleware,
        ))
}

// ======================================================================
// TraceSpan Tests
// ======================================================================

#[test]
fn test_trace_span_new_initializes_fields() {
    let trace = TraceSpan::new(
        "req-1".to_string(),
        "openai".to_string(),
        "gpt-4".to_string(),
        Some("auth-42".to_string()),
    );

    assert_eq!(trace.request_id, "req-1");
    assert_eq!(trace.provider, "openai");
    assert_eq!(trace.model, "gpt-4");
    assert_eq!(trace.auth_id, Some("auth-42".to_string()));
    assert!(trace.end_time.is_none());
    assert!(trace.status_code.is_none());
    assert!(trace.latency_ms.is_none());
    assert!(trace.error_message.is_none());
    assert!(!trace.is_streaming);
    assert!(trace.input_tokens.is_none());
    assert!(trace.output_tokens.is_none());
}

#[test]
fn test_trace_span_unique_trace_ids() {
    let trace1 = TraceSpan::new("a".into(), "p".into(), "m".into(), None);
    let trace2 = TraceSpan::new("b".into(), "p".into(), "m".into(), None);

    assert_ne!(trace1.trace_id, trace2.trace_id);
}

#[test]
fn test_trace_span_complete_sets_end_time_and_status() {
    let mut trace = TraceSpan::new("req-1".into(), "openai".into(), "gpt-4".into(), None);

    assert!(trace.end_time.is_none());
    assert!(trace.status_code.is_none());
    assert!(trace.latency_ms.is_none());

    trace.complete(200);

    assert!(trace.end_time.is_some());
    assert_eq!(trace.status_code, Some(200));
    assert!(trace.latency_ms.is_some());
    assert!(trace.is_success());
}

#[test]
fn test_trace_span_complete_overwrites_previous() {
    let mut trace = TraceSpan::new("req-1".into(), "openai".into(), "gpt-4".into(), None);

    trace.complete(500);
    assert_eq!(trace.status_code, Some(500));

    trace.complete(200);
    // Complete should overwrite, but behavior depends on implementation.
    // We verify it updates to the new status.
    assert_eq!(trace.status_code, Some(200));
}

#[test]
fn test_trace_span_is_success_various_codes() {
    let success_codes = [200u16, 201, 202, 204, 299];
    let failure_codes = [400u16, 401, 403, 404, 429, 500, 502, 503];

    for code in success_codes {
        let mut trace = TraceSpan::new("a".into(), "p".into(), "m".into(), None);
        trace.complete(code);
        assert!(trace.is_success(), "Expected {code} to be successful");
    }

    for code in failure_codes {
        let mut trace = TraceSpan::new("a".into(), "p".into(), "m".into(), None);
        trace.complete(code);
        assert!(!trace.is_success(), "Expected {code} to be a failure");
    }
}

#[test]
fn test_trace_span_set_error() {
    let mut trace = TraceSpan::new("req-err".into(), "openai".into(), "gpt-4".into(), None);

    trace.set_error("Rate limit exceeded".to_string());

    assert_eq!(trace.error_message, Some("Rate limit exceeded".to_string()));
    assert!(trace.end_time.is_some());
    assert_eq!(trace.status_code, Some(500));
    assert!(!trace.is_success());
}

#[test]
fn test_trace_span_set_error_overwrites_status() {
    let mut trace = TraceSpan::new("req-err".into(), "openai".into(), "gpt-4".into(), None);
    trace.complete(200);

    trace.set_error("Something went wrong".to_string());

    // set_error overrides the previous successful status
    assert_eq!(
        trace.error_message,
        Some("Something went wrong".to_string())
    );
    assert!(!trace.is_success());
}

#[test]
fn test_trace_span_is_streaming_flag() {
    let mut trace = TraceSpan::new("stream-1".into(), "openai".into(), "gpt-4".into(), None);
    assert!(!trace.is_streaming);

    trace.is_streaming = true;
    assert!(trace.is_streaming);
}

#[test]
fn test_trace_span_without_tokens() {
    let mut trace = TraceSpan::new("no-tokens".into(), "openai".into(), "gpt-4".into(), None);
    trace.complete(200);

    assert!(trace.input_tokens.is_none());
    assert!(trace.output_tokens.is_none());
    assert!(trace.is_success());
}

#[test]
fn test_trace_span_latency_calculation() {
    let mut trace = TraceSpan::new("latency-test".into(), "openai".into(), "gpt-4".into(), None);
    trace.start_time = Utc::now() - chrono::Duration::milliseconds(250);
    trace.complete(200);

    let latency = trace
        .latency_ms
        .expect("latency should be set after complete");
    // Allow some tolerance for test execution time
    assert!(latency >= 240, "Expected latency >= 240ms, got {latency}ms");
    assert!(latency <= 300, "Expected latency <= 300ms, got {latency}ms");
}

#[test]
fn test_trace_span_with_prompt() {
    let mut trace = TraceSpan::new("prompt-test".into(), "openai".into(), "gpt-4".into(), None);
    trace.prompt = Some("Hello, world!".to_string());
    trace.input_tokens = Some(5);
    trace.output_tokens = Some(20);
    trace.complete(200);

    assert_eq!(trace.prompt, Some("Hello, world!".to_string()));
    assert_eq!(trace.input_tokens, Some(5));
    assert_eq!(trace.output_tokens, Some(20));
}

#[test]
fn test_trace_span_without_auth_id() {
    let trace = TraceSpan::new("no-auth".into(), "openai".into(), "gpt-4".into(), None);
    assert!(trace.auth_id.is_none());
}

// ======================================================================
// MemoryTraceCollector Tests
// ======================================================================

#[tokio::test]
async fn test_collector_new_with_custom_size() {
    let collector = MemoryTraceCollector::new(42);
    assert_eq!(collector.trace_count().await, 0);
}

#[tokio::test]
async fn test_collector_with_default_size() {
    let collector = MemoryTraceCollector::with_default_size();
    assert_eq!(collector.trace_count().await, 0);

    // Fill up past default to verify it has a meaningful size
    for i in 0..500 {
        let trace = create_completed_trace(&format!("d-{i}"), "openai", "gpt-4", 50, 200);
        collector.record_trace(trace).await;
    }
    assert_eq!(collector.trace_count().await, 500);
}

#[tokio::test]
async fn test_collector_record_single_trace() {
    let collector = MemoryTraceCollector::new(100);
    let trace = create_completed_trace("solo", "openai", "gpt-4", 100, 200);

    collector.record_trace(trace).await;

    assert_eq!(collector.trace_count().await, 1);
    let traces = collector.get_traces().await;
    assert_eq!(traces[0].request_id, "solo");
}

#[tokio::test]
async fn test_collector_eviction_fifo_order() {
    let collector = MemoryTraceCollector::new(5);

    for i in 0..10 {
        let trace = create_completed_trace(&format!("fifo-{i}"), "openai", "gpt-4", 50, 200);
        collector.record_trace(trace).await;
    }

    assert_eq!(collector.trace_count().await, 5);
    let traces = collector.get_traces().await;
    assert_eq!(traces[0].request_id, "fifo-5");
    assert_eq!(traces[1].request_id, "fifo-6");
    assert_eq!(traces[4].request_id, "fifo-9");
}

#[tokio::test]
async fn test_collector_clear_empties_all() {
    let collector = MemoryTraceCollector::new(100);

    for i in 0..30 {
        let trace =
            create_completed_trace(&format!("clear-{i}"), "anthropic", "claude-3", 100, 200);
        collector.record_trace(trace).await;
    }
    assert_eq!(collector.trace_count().await, 30);

    collector.clear().await;
    assert_eq!(collector.trace_count().await, 0);

    let traces = collector.get_traces().await;
    assert!(traces.is_empty());
}

#[tokio::test]
async fn test_collector_trace_count_matches_length() {
    let collector = MemoryTraceCollector::new(200);

    for i in 0..75 {
        let trace = create_completed_trace(&format!("count-{i}"), "google", "gemini", 50, 200);
        collector.record_trace(trace).await;
    }

    let count = collector.trace_count().await;
    let traces = collector.get_traces().await;
    assert_eq!(count, 75);
    assert_eq!(traces.len(), 75);
}

#[tokio::test]
async fn test_collector_empty_traces() {
    let collector = MemoryTraceCollector::new(100);
    assert_eq!(collector.trace_count().await, 0);
    assert!(collector.get_traces().await.is_empty());
}

#[tokio::test]
async fn test_collector_single_capacity() {
    let collector = MemoryTraceCollector::new(1);

    let t1 = create_completed_trace("first", "openai", "gpt-4", 50, 200);
    collector.record_trace(t1).await;
    assert_eq!(collector.trace_count().await, 1);

    let t2 = create_completed_trace("second", "openai", "gpt-4", 50, 200);
    collector.record_trace(t2).await;
    assert_eq!(collector.trace_count().await, 1);

    let traces = collector.get_traces().await;
    assert_eq!(traces[0].request_id, "second");
}

#[tokio::test]
async fn test_collector_clear_and_refill_with_different_providers() {
    let collector = MemoryTraceCollector::new(50);

    for i in 0..20 {
        let trace = create_completed_trace(&format!("openai-{i}"), "openai", "gpt-4", 100, 200);
        collector.record_trace(trace).await;
    }

    collector.clear().await;

    for i in 0..15 {
        let trace =
            create_completed_trace(&format!("anthropic-{i}"), "anthropic", "claude-3", 150, 200);
        collector.record_trace(trace).await;
    }

    assert_eq!(collector.trace_count().await, 15);
    let traces = collector.get_traces().await;
    assert!(traces.iter().all(|t| t.provider == "anthropic"));
}

#[tokio::test]
async fn test_collector_preserves_trace_data() {
    let collector = MemoryTraceCollector::new(10);

    let mut trace = TraceSpan::new(
        "data-check".to_string(),
        "openai".to_string(),
        "gpt-4-turbo".to_string(),
        Some("org-123".to_string()),
    );
    trace.input_tokens = Some(500);
    trace.output_tokens = Some(250);
    trace.is_streaming = true;
    trace.prompt = Some("What is Rust?".to_string());
    trace.start_time = Utc::now() - chrono::Duration::milliseconds(300);
    trace.complete(200);

    collector.record_trace(trace).await;

    let traces = collector.get_traces().await;
    assert_eq!(traces.len(), 1);

    let t = &traces[0];
    assert_eq!(t.request_id, "data-check");
    assert_eq!(t.provider, "openai");
    assert_eq!(t.model, "gpt-4-turbo");
    assert_eq!(t.auth_id, Some("org-123".to_string()));
    assert_eq!(t.input_tokens, Some(500));
    assert_eq!(t.output_tokens, Some(250));
    assert!(t.is_streaming);
    assert_eq!(t.prompt, Some("What is Rust?".to_string()));
    assert!(t.is_success());
}

// ======================================================================
// TraceMetrics Tests
// ======================================================================

#[test]
fn test_metrics_new_is_empty() {
    let metrics = TraceMetrics::new();

    assert_eq!(metrics.total_requests, 0);
    assert_eq!(metrics.successful_requests, 0);
    assert_eq!(metrics.failed_requests, 0);
    assert_eq!(metrics.latency_count, 0);
    assert_eq!(metrics.avg_latency_ms, 0.0);
    assert_eq!(metrics.ewma_latency_ms, 0.0);
    assert_eq!(metrics.success_rate, 0.0);
    assert!(metrics.provider_metrics.is_empty());
    assert!(metrics.model_metrics.is_empty());
}

#[test]
fn test_metrics_update_single_successful_trace() {
    let mut metrics = TraceMetrics::new();
    let trace = create_completed_trace("ok-1", "openai", "gpt-4", 100, 200);

    metrics.update(&trace);

    assert_eq!(metrics.total_requests, 1);
    assert_eq!(metrics.successful_requests, 1);
    assert_eq!(metrics.failed_requests, 0);
    assert_eq!(metrics.latency_count, 1);
    assert!(metrics.avg_latency_ms > 0.0);
    assert!(metrics.ewma_latency_ms > 0.0);
    assert_eq!(metrics.success_rate, 1.0);
}

#[test]
fn test_metrics_update_single_failed_trace() {
    let mut metrics = TraceMetrics::new();
    let trace = create_completed_trace("fail-1", "openai", "gpt-4", 50, 500);

    metrics.update(&trace);

    assert_eq!(metrics.total_requests, 1);
    assert_eq!(metrics.successful_requests, 0);
    assert_eq!(metrics.failed_requests, 1);
    assert_eq!(metrics.success_rate, 0.0);
}

#[test]
fn test_metrics_update_error_trace() {
    let mut metrics = TraceMetrics::new();

    let mut trace = TraceSpan::new("err-1".into(), "anthropic".into(), "claude-3".into(), None);
    trace.set_error("Connection refused".to_string());

    metrics.update(&trace);

    assert_eq!(metrics.total_requests, 1);
    assert_eq!(metrics.failed_requests, 1);
    assert_eq!(metrics.successful_requests, 0);
}

#[test]
fn test_metrics_aggregate_empty() {
    let metrics = TraceMetrics::aggregate(&[]);
    assert_eq!(metrics.total_requests, 0);
}

#[test]
fn test_metrics_aggregate_multiple_traces() {
    let traces = vec![
        create_completed_trace("a1", "openai", "gpt-4", 100, 200),
        create_completed_trace("a2", "openai", "gpt-4", 200, 200),
        create_completed_trace("a3", "anthropic", "claude-3", 150, 200),
        create_completed_trace("a4", "anthropic", "claude-3", 250, 500),
        create_completed_trace("a5", "google", "gemini-pro", 80, 429),
        create_completed_trace("a6", "openai", "gpt-3.5", 300, 200),
        create_completed_trace("a7", "anthropic", "claude-3-opus", 120, 200),
    ];

    let metrics = TraceMetrics::aggregate(&traces);

    assert_eq!(metrics.total_requests, 7);
    assert_eq!(metrics.successful_requests, 5);
    assert_eq!(metrics.failed_requests, 2);
    assert_eq!(metrics.latency_count, 7);

    // Check success rate (5/7 ≈ 0.714)
    let expected_rate = 5.0 / 7.0;
    assert!(
        (metrics.success_rate - expected_rate).abs() < 0.01,
        "Expected ~{expected_rate}, got {}",
        metrics.success_rate
    );

    // Check average latency: (100+200+150+250+80+300+120) / 7 = 171.43
    let expected_latency = 1200.0 / 7.0;
    assert!(
        (metrics.avg_latency_ms - expected_latency).abs() < 1.0,
        "Expected ~{expected_latency}, got {}",
        metrics.avg_latency_ms
    );
}

#[test]
fn test_metrics_aggregate_provider_breakdown() {
    let traces = vec![
        create_completed_trace("p1", "openai", "gpt-4", 100, 200),
        create_completed_trace("p2", "openai", "gpt-4", 150, 500),
        create_completed_trace("p3", "openai", "gpt-3.5", 80, 200),
        create_completed_trace("p4", "anthropic", "claude-3", 200, 200),
        create_completed_trace("p5", "anthropic", "claude-3", 120, 200),
        create_completed_trace("p6", "google", "gemini", 90, 404),
    ];

    let metrics = TraceMetrics::aggregate(&traces);

    assert_eq!(metrics.provider_metrics.len(), 3);

    let openai = metrics.provider_metrics.get("openai").unwrap();
    assert_eq!(openai.total_requests, 3);
    assert_eq!(openai.successful_requests, 2);

    let anthropic = metrics.provider_metrics.get("anthropic").unwrap();
    assert_eq!(anthropic.total_requests, 2);
    assert_eq!(anthropic.successful_requests, 2);

    let google = metrics.provider_metrics.get("google").unwrap();
    assert_eq!(google.total_requests, 1);
    assert_eq!(google.successful_requests, 0);
}

#[test]
fn test_metrics_aggregate_model_breakdown_with_tokens() {
    let traces = vec![
        create_trace_with_tokens("m1", "openai", "gpt-4", 1000, 500, 200),
        create_trace_with_tokens("m2", "openai", "gpt-4", 800, 400, 200),
        create_trace_with_tokens("m3", "anthropic", "claude-3", 500, 300, 200),
        create_trace_with_tokens("m4", "anthropic", "claude-3", 0, 0, 500),
    ];

    let metrics = TraceMetrics::aggregate(&traces);

    assert_eq!(metrics.model_metrics.len(), 2);

    let gpt4 = metrics.model_metrics.get("gpt-4").unwrap();
    assert_eq!(gpt4.total_requests, 2);
    assert_eq!(gpt4.total_input_tokens, 1800);
    assert_eq!(gpt4.total_output_tokens, 900);

    let claude3 = metrics.model_metrics.get("claude-3").unwrap();
    assert_eq!(claude3.total_requests, 2);
    assert_eq!(claude3.total_input_tokens, 500);
    assert_eq!(claude3.total_output_tokens, 300);
}

#[test]
fn test_metrics_ewma_latency_updates() {
    let mut metrics = TraceMetrics::new();

    // First update: EWMA should equal the first value
    let t1 = create_completed_trace("ewma-1", "openai", "gpt-4", 100, 200);
    metrics.update(&t1);
    let ewma1 = metrics.ewma_latency_ms;

    // Second update: EWMA should shift toward the new value
    let t2 = create_completed_trace("ewma-2", "openai", "gpt-4", 200, 200);
    metrics.update(&t2);
    let ewma2 = metrics.ewma_latency_ms;

    // EWMA should have moved toward 200 but not reached it
    assert!(ewma2 > ewma1, "EWMA should increase toward 200");
    assert!(ewma2 < 200.0, "EWMA should not have jumped to 200");
}

#[test]
fn test_metrics_get_percentile_returns_none() {
    let metrics = TraceMetrics::new();
    assert!(metrics.get_percentile(50.0).is_none());

    // Also check after aggregation - it's a placeholder
    let traces = vec![
        create_completed_trace("p-1", "openai", "gpt-4", 100, 200),
        create_completed_trace("p-2", "openai", "gpt-4", 200, 200),
    ];
    let metrics = TraceMetrics::aggregate(&traces);
    assert!(metrics.get_percentile(50.0).is_none());
}

#[test]
fn test_metrics_update_without_latency() {
    let mut metrics = TraceMetrics::new();

    let mut trace = TraceSpan::new("no-latency".into(), "openai".into(), "gpt-4".into(), None);
    // Don't set start_time in the past, so complete won't produce meaningful latency
    trace.complete(200);

    metrics.update(&trace);

    assert_eq!(metrics.total_requests, 1);
    // Latency may or may not be recorded depending on timing, but should not panic
}

#[test]
fn test_metrics_incremental_updates_match_aggregate() {
    let traces = vec![
        create_completed_trace("iu-1", "openai", "gpt-4", 100, 200),
        create_completed_trace("iu-2", "openai", "gpt-4", 150, 500),
        create_completed_trace("iu-3", "anthropic", "claude-3", 200, 200),
    ];

    // Method 1: incremental updates
    let mut incremental = TraceMetrics::new();
    for trace in &traces {
        incremental.update(trace);
    }

    // Method 2: aggregate
    let aggregated = TraceMetrics::aggregate(&traces);

    // Total counts should match
    assert_eq!(incremental.total_requests, aggregated.total_requests);
    assert_eq!(
        incremental.successful_requests,
        aggregated.successful_requests
    );
    assert_eq!(incremental.failed_requests, aggregated.failed_requests);
}

// ======================================================================
// ProviderMetrics Tests
// ======================================================================

#[test]
fn test_provider_metrics_success_rate_all_success() {
    let traces = vec![
        create_completed_trace("ps-1", "openai", "gpt-4", 100, 200),
        create_completed_trace("ps-2", "openai", "gpt-4", 150, 201),
        create_completed_trace("ps-3", "openai", "gpt-3.5", 80, 204),
    ];

    let metrics = TraceMetrics::aggregate(&traces);
    let openai = metrics.provider_metrics.get("openai").unwrap();
    assert_eq!(openai.success_rate(), 1.0);
}

#[test]
fn test_provider_metrics_success_rate_all_failures() {
    let traces = vec![
        create_completed_trace("pf-1", "openai", "gpt-4", 100, 500),
        create_completed_trace("pf-2", "openai", "gpt-4", 150, 503),
    ];

    let metrics = TraceMetrics::aggregate(&traces);
    let openai = metrics.provider_metrics.get("openai").unwrap();
    assert_eq!(openai.success_rate(), 0.0);
}

#[test]
fn test_provider_metrics_latency() {
    let traces = vec![
        create_completed_trace("pl-1", "anthropic", "claude-3", 200, 200),
        create_completed_trace("pl-2", "anthropic", "claude-3", 400, 200),
    ];

    let metrics = TraceMetrics::aggregate(&traces);
    let anthropic = metrics.provider_metrics.get("anthropic").unwrap();

    assert_eq!(anthropic.latency_count, 2);
    assert!(anthropic.avg_latency_ms > 0.0);
    assert!(anthropic.ewma_latency_ms > 0.0);
}

#[test]
fn test_provider_metrics_empty() {
    let metrics = TraceMetrics::new();
    assert!(metrics.provider_metrics.is_empty());
}

// ======================================================================
// ModelMetrics Tests
// ======================================================================

#[test]
fn test_model_metrics_success_rate() {
    let traces = vec![
        create_trace_with_tokens("ms-1", "openai", "gpt-4", 100, 50, 200),
        create_trace_with_tokens("ms-2", "openai", "gpt-4", 200, 100, 500),
        create_trace_with_tokens("ms-3", "openai", "gpt-4", 80, 40, 200),
    ];

    let metrics = TraceMetrics::aggregate(&traces);
    let gpt4 = metrics.model_metrics.get("gpt-4").unwrap();
    assert_eq!(gpt4.success_rate(), 2.0 / 3.0);
}

#[test]
fn test_model_metrics_avg_total_tokens() {
    let traces = vec![
        create_trace_with_tokens("mt-1", "openai", "gpt-4", 100, 50, 200),
        create_trace_with_tokens("mt-2", "openai", "gpt-4", 200, 100, 200),
        create_trace_with_tokens("mt-3", "openai", "gpt-4", 300, 150, 200),
    ];

    let metrics = TraceMetrics::aggregate(&traces);
    let gpt4 = metrics.model_metrics.get("gpt-4").unwrap();

    // Total input: 600, total output: 300, requests: 3
    // avg_total = (600 + 300) / 3 = 300
    assert_eq!(gpt4.total_input_tokens, 600);
    assert_eq!(gpt4.total_output_tokens, 300);
    assert_eq!(gpt4.total_requests, 3);
    assert_eq!(gpt4.avg_total_tokens(), 300.0);
}

#[test]
fn test_model_metrics_avg_total_tokens_no_requests() {
    // Aggregate from empty traces to get a model with zero requests
    // We create a trace with zero tokens and verify avg_total_tokens works
    let mut trace = TraceSpan::new("zero".into(), "openai".into(), "gpt-4".into(), None);
    trace.input_tokens = Some(0);
    trace.output_tokens = Some(0);
    trace.complete(200);

    let metrics = TraceMetrics::aggregate(&[trace]);
    let gpt4 = metrics.model_metrics.get("gpt-4").unwrap();
    assert_eq!(gpt4.total_requests, 1);
    assert_eq!(gpt4.total_input_tokens, 0);
    assert_eq!(gpt4.total_output_tokens, 0);
    assert_eq!(gpt4.avg_total_tokens(), 0.0);
}

#[test]
fn test_model_metrics_with_partial_tokens() {
    let traces = vec![
        create_trace_with_tokens("pt-1", "openai", "gpt-4", 100, 50, 200),
        {
            // Trace with only input tokens
            let mut t = TraceSpan::new("pt-2".into(), "openai".into(), "gpt-4".into(), None);
            t.input_tokens = Some(200);
            t.complete(200);
            t
        },
    ];

    let metrics = TraceMetrics::aggregate(&traces);
    let gpt4 = metrics.model_metrics.get("gpt-4").unwrap();
    assert_eq!(gpt4.total_input_tokens, 300);
    assert_eq!(gpt4.total_output_tokens, 50); // Only first trace has output
}

// ======================================================================
// TracingMiddleware Tests
// ======================================================================

#[tokio::test]
async fn test_middleware_extracts_headers_correctly() {
    let collector = Arc::new(MemoryTraceCollector::new(100));
    let middleware = TracingMiddleware::new(collector.clone());
    let app = make_test_app(middleware);

    let _response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/ok")
                .header("x-request-id", "custom-id-42")
                .header("x-llm-provider", "anthropic")
                .header("x-llm-model", "claude-3-opus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let traces = collector.get_traces().await;
    assert_eq!(traces.len(), 1);
    assert_eq!(traces[0].request_id, "custom-id-42");
    assert_eq!(traces[0].provider, "anthropic");
    assert_eq!(traces[0].model, "claude-3-opus");
}

#[tokio::test]
async fn test_middleware_generates_request_id_when_missing() {
    let collector = Arc::new(MemoryTraceCollector::new(100));
    let middleware = TracingMiddleware::new(collector.clone());
    let app = make_test_app(middleware);

    let _response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/ok")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let traces = collector.get_traces().await;
    assert_eq!(traces.len(), 1);
    // Should have auto-generated a request ID (non-empty)
    assert!(!traces[0].request_id.is_empty());
}

#[tokio::test]
async fn test_middleware_handles_post_requests() {
    let collector = Arc::new(MemoryTraceCollector::new(100));
    let middleware = TracingMiddleware::new(collector.clone());
    let app = make_test_app(middleware);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/created")
                .header("x-request-id", "post-1")
                .body(Body::from("test body"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let traces = collector.get_traces().await;
    assert_eq!(traces.len(), 1);
    assert_eq!(traces[0].request_id, "post-1");
    assert!(traces[0].is_success());
}

#[tokio::test]
async fn test_middleware_various_error_codes() {
    let error_scenarios = vec![
        ("/error", StatusCode::INTERNAL_SERVER_ERROR, 500u16),
        ("/not-found", StatusCode::NOT_FOUND, 404u16),
        ("/rate-limit", StatusCode::TOO_MANY_REQUESTS, 429u16),
    ];

    for (path, expected_status, expected_code) in error_scenarios {
        let collector = Arc::new(MemoryTraceCollector::new(100));
        let middleware = TracingMiddleware::new(collector.clone());
        let app = make_test_app(middleware);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(path)
                    .header("x-request-id", format!("req-{expected_code}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), expected_status, "Path: {path}");

        let traces = collector.get_traces().await;
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0].status_code, Some(expected_code));
        assert!(!traces[0].is_success());
    }
}

#[tokio::test]
async fn test_middleware_without_provider_model_headers() {
    let collector = Arc::new(MemoryTraceCollector::new(100));
    let middleware = TracingMiddleware::new(collector.clone());
    let app = make_test_app(middleware);

    let _response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/ok")
                .header("x-request-id", "no-headers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let traces = collector.get_traces().await;
    assert_eq!(traces.len(), 1);
    // Provider and model should default to "unknown" when headers are missing
    assert_eq!(traces[0].provider, "unknown");
    assert_eq!(traces[0].model, "unknown");
}

#[tokio::test]
async fn test_middleware_records_latency() {
    let collector = Arc::new(MemoryTraceCollector::new(100));
    let middleware = TracingMiddleware::new(collector.clone());
    let app = make_test_app(middleware);

    let _response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/ok")
                .header("x-request-id", "latency-req")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let traces = collector.get_traces().await;
    assert_eq!(traces.len(), 1);
    assert!(
        traces[0].latency_ms.is_some(),
        "Middleware should record latency"
    );
}

#[tokio::test]
async fn test_middleware_multiple_sequential_requests() {
    let collector = Arc::new(MemoryTraceCollector::new(100));
    let middleware = TracingMiddleware::new(collector.clone());
    let app = make_test_app(middleware);

    for i in 0..10 {
        let _response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/ok")
                    .header("x-request-id", format!("seq-{i}"))
                    .header("x-llm-provider", "openai")
                    .header("x-llm-model", "gpt-4")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    let traces = collector.get_traces().await;
    assert_eq!(traces.len(), 10);

    // Verify all are successful
    assert!(traces.iter().all(llm_tracing::TraceSpan::is_success));

    // Verify request IDs
    for (i, trace) in traces.iter().enumerate() {
        assert_eq!(trace.request_id, format!("seq-{i}"));
    }
}

// ======================================================================
// TracingMiddlewareBuilder Tests
// ======================================================================

#[test]
fn test_builder_new_and_default() {
    // Verify both new() and default() produce builders that can be used
    let collector1 = Arc::new(MemoryTraceCollector::new(10)) as Arc<dyn TraceCollector>;
    let collector2 = Arc::new(MemoryTraceCollector::new(10)) as Arc<dyn TraceCollector>;

    let _m1 = TracingMiddlewareBuilder::new()
        .with_collector(collector1)
        .build();
    let _m2 = TracingMiddlewareBuilder::default()
        .with_collector(collector2)
        .build();

    // Both should build without panicking - that's the main verification
}

#[tokio::test]
async fn test_builder_with_collector_works_end_to_end() {
    let collector = Arc::new(MemoryTraceCollector::new(50)) as Arc<dyn TraceCollector>;

    let middleware = TracingMiddlewareBuilder::new()
        .with_collector(collector.clone())
        .build();

    let app = make_test_app(middleware);

    let _response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/ok")
                .header("x-request-id", "builder-test")
                .header("x-llm-provider", "google")
                .header("x-llm-model", "gemini-pro")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(collector.trace_count().await, 1);
    let traces = collector.get_traces().await;
    assert_eq!(traces[0].provider, "google");
    assert_eq!(traces[0].model, "gemini-pro");
}

// ======================================================================
// End-to-End Integration Tests
// ======================================================================

#[tokio::test]
async fn test_e2e_multiple_providers_and_models() {
    let collector = Arc::new(MemoryTraceCollector::new(200));
    let middleware = TracingMiddleware::new(collector.clone());
    let app = make_test_app(middleware);

    let scenarios = [
        ("openai", "gpt-4"),
        ("openai", "gpt-3.5-turbo"),
        ("anthropic", "claude-3-opus"),
        ("anthropic", "claude-3-sonnet"),
        ("google", "gemini-pro"),
        ("google", "gemini-ultra"),
    ];

    for (i, (provider, model)) in scenarios.iter().enumerate() {
        let _response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/ok")
                    .header("x-request-id", format!("e2e-{i}"))
                    .header("x-llm-provider", *provider)
                    .header("x-llm-model", *model)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    let traces = collector.get_traces().await;
    assert_eq!(traces.len(), 6);

    let metrics = TraceMetrics::aggregate(&traces);
    assert_eq!(metrics.total_requests, 6);
    assert_eq!(metrics.successful_requests, 6);
    assert_eq!(metrics.failed_requests, 0);
    assert_eq!(metrics.provider_metrics.len(), 3);
    assert_eq!(metrics.model_metrics.len(), 6);
}

#[tokio::test]
async fn test_e2e_mixed_success_failure_metrics() {
    let collector = Arc::new(MemoryTraceCollector::new(200));
    let middleware = TracingMiddleware::new(collector.clone());
    let app = make_test_app(middleware);

    // 5 successful, 3 failures
    for i in 0..5 {
        let _response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/ok")
                    .header("x-request-id", format!("mix-ok-{i}"))
                    .header("x-llm-provider", "openai")
                    .header("x-llm-model", "gpt-4")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    for i in 0..3 {
        let _response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/error")
                    .header("x-request-id", format!("mix-err-{i}"))
                    .header("x-llm-provider", "anthropic")
                    .header("x-llm-model", "claude-3")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    let traces = collector.get_traces().await;
    assert_eq!(traces.len(), 8);

    let metrics = TraceMetrics::aggregate(&traces);
    assert_eq!(metrics.total_requests, 8);
    assert_eq!(metrics.successful_requests, 5);
    assert_eq!(metrics.failed_requests, 3);

    let openai = metrics.provider_metrics.get("openai").unwrap();
    assert_eq!(openai.total_requests, 5);
    assert_eq!(openai.successful_requests, 5);

    let anthropic = metrics.provider_metrics.get("anthropic").unwrap();
    assert_eq!(anthropic.total_requests, 3);
    assert_eq!(anthropic.successful_requests, 0);
}

#[tokio::test]
async fn test_e2e_collector_eviction_during_requests() {
    let collector = Arc::new(MemoryTraceCollector::new(5));
    let middleware = TracingMiddleware::new(collector.clone());
    let app = make_test_app(middleware);

    // Send 10 requests to a collector with capacity 5
    for i in 0..10 {
        let _response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/ok")
                    .header("x-request-id", format!("evict-{i}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Only the last 5 should be retained
    assert_eq!(collector.trace_count().await, 5);
    let traces = collector.get_traces().await;
    assert_eq!(traces[0].request_id, "evict-5");
    assert_eq!(traces[4].request_id, "evict-9");
}

// ======================================================================
// Concurrent Access Tests
// ======================================================================

#[tokio::test]
async fn test_concurrent_writers_high_contention() {
    let collector = Arc::new(MemoryTraceCollector::new(2000));
    let mut tasks = JoinSet::new();

    for i in 0..100 {
        let collector = collector.clone();
        tasks.spawn(async move {
            for j in 0..50 {
                let trace = create_completed_trace(
                    &format!(" contention-{i}-{j}"),
                    "openai",
                    "gpt-4",
                    50 + (j as u64),
                    if j % 5 == 0 { 500 } else { 200 },
                );
                collector.record_trace(trace).await;
            }
        });
    }

    while tasks.join_next().await.is_some() {}

    assert_eq!(collector.trace_count().await, 2000);

    let traces = collector.get_traces().await;
    let metrics = TraceMetrics::aggregate(&traces);
    assert_eq!(metrics.total_requests, 2000);
}

#[tokio::test]
async fn test_concurrent_read_write_mixed() {
    let collector = Arc::new(MemoryTraceCollector::new(300));
    let mut tasks = JoinSet::new();

    // Writers
    for i in 0..30 {
        let collector = collector.clone();
        tasks.spawn(async move {
            for j in 0..30 {
                let trace =
                    create_completed_trace(&format!("rw-{i}-{j}"), "openai", "gpt-4", 100, 200);
                collector.record_trace(trace).await;
                tokio::time::sleep(Duration::from_micros(5)).await;
            }
        });
    }

    // Readers - verify consistency
    for _ in 0..30 {
        let collector = collector.clone();
        tasks.spawn(async move {
            for _ in 0..30 {
                let count = collector.trace_count().await;
                assert!(count <= 300, "Count {count} exceeded capacity");
                let traces = collector.get_traces().await;
                assert_eq!(
                    traces.len(),
                    count,
                    "Trace count mismatch with get_traces length"
                );
                tokio::time::sleep(Duration::from_micros(5)).await;
            }
        });
    }

    while tasks.join_next().await.is_some() {}

    let final_count = collector.trace_count().await;
    assert!(final_count <= 300);
    let traces = collector.get_traces().await;
    assert_eq!(traces.len(), final_count);
}

#[tokio::test]
async fn test_concurrent_clear_while_writing() {
    let collector = Arc::new(MemoryTraceCollector::new(500));
    let mut tasks = JoinSet::new();

    // Writers
    for i in 0..10 {
        let collector = collector.clone();
        tasks.spawn(async move {
            for j in 0..100 {
                let trace =
                    create_completed_trace(&format!("cw-{i}-{j}"), "openai", "gpt-4", 50, 200);
                collector.record_trace(trace).await;
                tokio::time::sleep(Duration::from_micros(10)).await;
            }
        });
    }

    // Clearers
    for _ in 0..5 {
        let collector = collector.clone();
        tasks.spawn(async move {
            for _ in 0..5 {
                tokio::time::sleep(Duration::from_millis(5)).await;
                collector.clear().await;
            }
        });
    }

    while tasks.join_next().await.is_some() {}

    // After all tasks complete, the state should be consistent
    let count = collector.trace_count().await;
    let traces = collector.get_traces().await;
    assert_eq!(traces.len(), count);
    assert!(count <= 500);
}

// ======================================================================
// Stress Tests
// ======================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_stress_high_throughput_varied_providers() {
    let collector = Arc::new(MemoryTraceCollector::new(10000));
    let mut tasks = JoinSet::new();

    let providers = vec![
        ("openai", "gpt-4"),
        ("openai", "gpt-3.5-turbo"),
        ("anthropic", "claude-3-opus"),
        ("anthropic", "claude-3-sonnet"),
        ("google", "gemini-pro"),
    ];

    for task_id in 0..50 {
        let collector = collector.clone();
        let providers = providers.clone();
        tasks.spawn(async move {
            for i in 0..200 {
                let (provider, model) = providers[task_id % providers.len()];
                let trace = create_completed_trace(
                    &format!("stress-{task_id}-{i}"),
                    provider,
                    model,
                    20 + ((i * 3) % 100) as u64,
                    if i % 20 == 0 { 500 } else { 200 },
                );
                collector.record_trace(trace).await;
            }
        });
    }

    while tasks.join_next().await.is_some() {}

    assert_eq!(collector.trace_count().await, 10000);

    let traces = collector.get_traces().await;
    let metrics = TraceMetrics::aggregate(&traces);
    assert_eq!(metrics.total_requests, 10000);
    assert_eq!(metrics.provider_metrics.len(), 3);
    assert_eq!(metrics.model_metrics.len(), 5);
    // 5% failure rate (i % 20 == 0)
    assert!(metrics.success_rate > 0.90);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_stress_rapid_fire_and_metrics() {
    let collector = Arc::new(MemoryTraceCollector::new(5000));

    // Rapidly insert traces
    for i in 0..5000 {
        let trace = create_completed_trace(
            &format!("rapid-{i}"),
            "openai",
            "gpt-4",
            10 + (i as u64) % 200,
            if i % 7 == 0 { 429 } else { 200 },
        );
        collector.record_trace(trace).await;
    }

    assert_eq!(collector.trace_count().await, 5000);

    let traces = collector.get_traces().await;
    let metrics = TraceMetrics::aggregate(&traces);

    assert_eq!(metrics.total_requests, 5000);
    assert!(metrics.avg_latency_ms > 0.0);
    assert!(metrics.ewma_latency_ms > 0.0);

    let openai = metrics.provider_metrics.get("openai").unwrap();
    assert_eq!(openai.total_requests, 5000);
    // 1/7 ≈ 14.3% failure rate
    assert!(openai.success_rate() > 0.80);
    assert!(openai.success_rate() < 0.90);
}
