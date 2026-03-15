//! Request/response observability for LLM requests.
//!
//! Captures trace spans with timing, token usage, and error information.
//! Integrates with Axum as middleware and aggregates metrics per model
//! and provider for monitoring and cost analysis.

pub mod collector;
pub mod metrics;
pub mod middleware;
pub mod trace;

pub use collector::{MemoryTraceCollector, TraceCollector};
pub use metrics::{ModelMetrics, ProviderMetrics, TraceMetrics};
pub use middleware::{tracing_middleware, TracingMiddleware, TracingMiddlewareBuilder};
pub use trace::TraceSpan;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("trace collection error: {0}")]
    Collection(String),
}

pub type Result<T> = std::result::Result<T, Error>;
