# model-registry

Model classification and registry for LLM providers.

## Purpose

Provides model metadata, categorization, and caching for intelligent routing decisions.

## Key Components

| File | Description |
|------|-------------|
| `categories.rs` | Tier, capability, cost, context, provider categorization |
| `registry.rs` | Central model registry with trait-based fetchers |
| `info.rs` | `ModelInfo`, `ModelCapabilities`, `RateLimits` types |
| `fetcher.rs` | `ModelFetcher` trait and `StaticFetcher` implementation |

## Classification Dimensions

| Dimension | Categories | Purpose |
|-----------|------------|---------|
| Capability | Vision, Tools, Streaming, Thinking | Feature matching |
| Tier | Flagship, Standard, Fast | Quality routing |
| Cost | UltraPremium, Premium, Standard, Economy | Cost optimization |
| Context | Small, Medium, Large, Ultra | Context window fitting |
| Provider | Anthropic, OpenAI, Google | Vendor routing |

## Usage

```rust
use model_registry::{Registry, ModelInfo, ModelCategorization, CapabilityCategory};

let registry = Registry::new();

// Get model info
let model = registry.get("claude-sonnet-4-20250514").await?;

// Filter by capability
let vision_models = registry.filter_by_capability(CapabilityCategory::Vision).await;

// Find cheapest model for context
let best = registry.find_best_fit(100_000).await;
```

## Dependencies

- `async-trait` - Async fetcher trait
- `chrono` - Cache TTL timestamps
- `serde` - JSON serialization
- `thiserror` - Error types
- `tokio` - Async runtime

## Tests

```bash
cargo test -p model-registry
```
