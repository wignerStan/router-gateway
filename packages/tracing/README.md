# llm-tracing

Request/response observability for LLM API calls.

## Purpose

Provides tracing, metrics collection, and middleware for observability of LLM API interactions.

## Key Components

| File | Description |
|------|-------------|
| `lib.rs` | Module exports |
| `trace.rs` | Trace context and span management |
| `collector.rs` | Metrics collection interface |
| `metrics.rs` | Performance metrics types |
| `middleware.rs` | Tower/Axum middleware |

## Usage

```rust
use llm_tracing::{TraceCollector, RequestMetrics};

// Create collector
let collector = TraceCollector::new();

// Record request
let trace = collector.start_request("claude-sonnet-4", &request).await;
// ... make API call ...
collector.end_request(trace, &response).await;

// Get metrics
let metrics = collector.get_metrics().await;
```

## Dependencies

- `tokio` - Async runtime
- `tower` - Middleware infrastructure
- `axum` - HTTP integration
- `tracing` - Span/tracing support
- `serde` - Serialization
- `chrono` - Timestamps
- `uuid` - Trace IDs

## Tests

```bash
cargo test -p llm-tracing
```
