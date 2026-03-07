# AGENTS.md

## Project Overview

A **local LLM gateway** written in Rust for intelligent request routing. Routes LLM requests to optimal credentials based on health, latency, and success rate. Designed for local development and self-hosted deployments.

**Stack**: Rust 1.75+, Tokio, Axum, SQLite
**Focus**: Smart routing, model registry, LLM tracing

## Package Names

> **Note:** Directory names differ from Cargo package names:

| Directory | Package Name |
|-----------|--------------|
| `packages/tracing/` | `llm-tracing` |
| `apps/cli/` | `my-cli` |

## Quick Start

| Task | Command |
|------|---------|
| Build | `cargo build` |
| Test | `cargo test` |
| Run Gateway | `cargo run -p gateway` |
| Lint | `cargo clippy -- -D warnings` |
| Format | `cargo fmt` |
| All Checks | `just qa` |

## Project Structure

```
gateway/
  packages/
    model-registry/   # Model metadata, 5-dimension categorization
    smart-routing/    # Weighted credential selection, health tracking
    tracing/          # Request/response observability (llm-tracing)
  apps/
    gateway/          # HTTP API server (Axum on :3000)
    cli/              # CLI management utility
```

## Architecture

### Smart Routing (`smart-routing`)

Policy-based credential selection with configurable weights and strategies.

**Routing Strategies** (configured via `SmartRoutingConfig.strategy`):
- `weighted` - Weighted random selection based on scores
- `time_aware` - Time-based credential preference (peak/off-peak)
- `quota_aware` - Quota-balanced selection with reserve ratio
- `adaptive` - Dynamically adjusts based on real-time metrics

**Weight Factors** (configured via `WeightConfig`):
| Factor | Default | Purpose |
|--------|---------|---------|
| success_rate | 0.35 | Favor credentials with higher success rate |
| latency | 0.25 | Favor lower latency credentials |
| health | 0.20 | Prefer healthy over degraded/unhealthy |
| load | 0.15 | Balance load across credentials |
| priority | 0.05 | Manual priority override |

**Health States**: `Healthy` → `Degraded` (429/503) → `Unhealthy` (401-403/500/502/504)

Key types: `SmartRoutingConfig`, `WeightConfig`, `HealthManager`, `WeightCalculator`

### Model Registry (`model-registry`)

Multi-dimension categorization for routing decisions:

| Dimension | Categories | Purpose |
|-----------|------------|---------|
| **Capability** | Vision, Tools, Streaming, Thinking | Feature matching |
| **Tier** | Flagship, Standard, Fast | Quality routing |
| **Cost** | UltraPremium, Premium, Standard, Economy | Cost optimization |
| **Context** | Small (<32K) → Ultra (500K+) | Context fitting |
| **Provider** | 20+ providers (Anthropic, OpenAI, Google, xAI, DeepSeek, Mistral, etc.) | Vendor routing |
| **Modality** | Text, Image, Audio, Video, Embedding, Code | Multi-modal routing |

**Supported Providers**: Anthropic, OpenAI, Google, xAI (Grok), DeepSeek, Mistral, Cohere, Perplexity, Alibaba (Qwen), Zhipu (GLM), Baidu, Moonshot (Kimi), ByteDance, Meta (Llama), Amazon Bedrock, Azure, and more.

**Routing Policy System**: Fine-grained routing rules based on multi-dimensional classification with:
- Policy filters (capabilities, tiers, costs, providers, modalities)
- Policy actions (prefer, avoid, block, weight)
- Conditional application (time-based, tenant-based, token-count)

Key types: `ModelInfo`, `ModelCategorization` trait, `Registry`, `RoutingPolicy`, `PolicyRegistry`

### LLM Tracing (`llm-tracing`)

Request/response observability for debugging and analytics:

- `TraceSpan`: Captures request ID, provider, model, tokens, latency, errors
- `MemoryTraceCollector`: In-memory trace storage
- `TracingMiddleware`: Axum middleware integration

Key types: `TraceSpan`, `TraceCollector` trait, `TracingMiddleware`

## Known Pitfalls

- All async operations use Tokio - always `.await` on registry/selector methods
- `Registry::get()` requires model ID to be non-empty (returns error otherwise)
- Health manager clones have independent storage (not shared state)
- Trace collectors are not shared across clones (same pattern)
- SQLite store requires `bundled` feature for cross-platform builds
- Model registry cache TTL is 1 hour by default

## Reference Implementations

| Pattern | Location |
|---------|----------|
| SmartRoutingConfig | `packages/smart-routing/src/config.rs:4-21` |
| WeightConfig defaults | `packages/smart-routing/src/config.rs:134-147` |
| Weight calculation | `packages/smart-routing/src/weight.rs:112-166` |
| Health state machine | `packages/smart-routing/src/health.rs:4-50` |
| Model categorization | `packages/model-registry/src/categories.rs:146-277` |
| Trace span | `packages/tracing/src/trace.rs:5-35` |
| HTTP endpoint setup | `apps/gateway/src/main.rs:43-48` |

## Documentation

- Model Classification: `docs/MODEL_CLASSIFICATION.md`
- API Transformation: `docs/API_TRANSFORMATION.md`
- Skills: `.agents/skills/`
