//! Request/response observability for LLM requests.
//!
//! Captures trace spans with timing, token usage, and error information.
//! Integrates with Axum as middleware and aggregates metrics per model
//! and provider for monitoring and cost analysis.

/// In-memory trace collection with bounded buffering.
pub mod collector;
/// Metrics aggregation for traces, per-provider and per-model.
pub mod metrics;
/// Axum middleware for automatic request tracing.
pub mod middleware;
/// Trace span data types for LLM requests.
pub mod trace;

pub use collector::{MemoryTraceCollector, TraceCollector};
pub use metrics::{ModelMetrics, ProviderMetrics, TraceMetrics};
pub use middleware::{tracing_middleware, TracingMiddleware, TracingMiddlewareBuilder};
pub use trace::TraceSpan;

/// Errors produced during trace collection and processing.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A trace could not be collected.
    #[error("trace collection error: {0}")]
    Collection(String),
}

/// Result type for fallible operations in this crate.
pub type Result<T> = std::result::Result<T, Error>;
