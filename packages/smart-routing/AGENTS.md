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
