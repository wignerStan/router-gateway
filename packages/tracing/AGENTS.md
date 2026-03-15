# llm-tracing

This project is part of the workspace. Please refer to the root [AGENTS.md](../../AGENTS.md) for global project guidelines and best practices.

## Crate-Specific Instructions

- **Location:** `packages/tracing`
- **Package:** `llm-tracing`
- **Build:** Run `cargo build -p llm-tracing`
- **Test:** Run `cargo test -p llm-tracing`

## Key Facts

- 4 modules: collector, metrics, middleware, trace
- TraceSpan captures: request ID, provider, model, tokens, latency, errors
- MemoryTraceCollector for in-memory trace storage
- TracingMiddleware for Axum integration
- Leaf package — no internal workspace dependencies
