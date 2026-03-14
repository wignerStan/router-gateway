//! Request/response observability for LLM requests.
//!
//! Captures trace spans with timing, token usage, and error information.
//! Integrates with Axum as middleware and aggregates metrics per model
//! and provider for monitoring and cost analysis.

/// Trace collector trait and in-memory implementation.
pub mod collector;
/// Metrics aggregation for LLM request traces.
pub mod metrics;
/// Axum middleware for automatic LLM request/response tracing.
pub mod middleware;
/// Individual trace span representation.
pub mod trace;

pub use collector::{MemoryTraceCollector, TraceCollector};
pub use metrics::{ModelMetrics, ProviderMetrics, TraceMetrics};
pub use middleware::{tracing_middleware, TracingMiddleware, TracingMiddlewareBuilder};
pub use trace::TraceSpan;

/// Errors produced by trace collection operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Trace collection failed with an underlying message.
    #[error("trace collection error: {0}")]
    Collection(String),
}

/// Result type for trace operations.
pub type Result<T> = std::result::Result<T, Error>;
