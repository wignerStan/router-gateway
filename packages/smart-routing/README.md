# smart-routing

Intelligent credential selection for LLM API routing.

## Purpose

Provides weighted credential selection based on health, latency, success rate, and quota status.

## Key Components

| File          | Description                                         |
| ------------- | --------------------------------------------------- |
| `router.rs`   | Main router structure                               |
| `selector.rs` | `SmartSelector` for credential selection            |
| `health.rs`   | Health state machine and tracking                   |
| `weight.rs`   | `WeightCalculator` trait and default implementation |
| `metrics.rs`  | `MetricsCollector` for performance tracking         |
| `config.rs`   | Configuration types                                 |
| `sqlite/`     | SQLite persistence layer                            |

## Routing Algorithm

Default weight calculation (configurable via `WeightConfig`):

```
weight = success_rate(0.35) + latency(0.25) + health(0.20) + load(0.15) + priority(0.05)
```

## Health States

```
Healthy -> Degraded (429/503) -> Unhealthy (401-403/500/502/504)
```

- **Healthy**: Normal operation
- **Degraded**: Temporary issues, reduced weight
- **Unhealthy**: Auth failures or server errors, cooldown applies (default: 60s)

## Usage

```rust
use smart_routing::{SmartSelector, HealthManager, SQLiteSelector};

// Create selector with SQLite persistence
let selector = SQLiteSelector::new(":memory:")?;

// Get best credential
let credential = selector.select_best(&requirements).await?;

// Update health status
selector.mark_success(&auth_id).await?;
selector.mark_failure(&auth_id, status_code).await?;
```

## Dependencies

- `tokio` - Async runtime
- `serde` - Configuration serialization
- `thiserror` - Error types
- `rusqlite` - SQLite persistence (with `bundled` feature)

## Tests

```bash
cargo test -p smart-routing
```
