# smart-routing

This project is part of the workspace. Please refer to the root [AGENTS.md](../../AGENTS.md) for global project guidelines and best practices.

## Crate-Specific Instructions

- **Location:** `packages/smart-routing`
- **Package:** `smart-routing`
- **Build:** Run `cargo build -p smart-routing`
- **Test:** Run `cargo test -p smart-routing`

## Key Facts

- 19 modules: bandit, candidate, classification, config, executor, fallback, filtering, health, history, metrics, outcome, policy_weight, reasoning, router, selector, session, sqlite, statistics, utility, weight
- Five routing strategies: weighted, time_aware, quota_aware, adaptive, policy_aware
- Depends on `model-registry` (internal) and `rusqlite` with `bundled` feature
- SQLite persistence for metrics and health data
- Thompson sampling bandit for exploration/exploitation
- Session affinity for multi-turn conversations
- Request classification: format detection, token estimation, content type detection

## Known Pitfalls

- All async operations use Tokio — always `.await` on registry/selector methods
- Health manager clones have independent storage (not shared state) — mutate via `&mut self`, not clone-then-mutate
- Trace collectors are not shared across clones (same pattern as health manager)
- SQLite store requires `bundled` feature for cross-platform builds
- Floating-point metrics may contain NaN — always use `partial_cmp().unwrap_or(Ordering::Equal)`
- Many modules were extracted from larger files — public API is re-exported through `mod.rs`

## Error Types

```rust
pub enum Error {
    Sqlite(#[from] SqliteError),  // Database operation failure
    Config(String),               // Configuration error
    NoCandidates,                 // No credentials available for routing
}
```
