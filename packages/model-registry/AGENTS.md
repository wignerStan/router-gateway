# model-registry

This project is part of the workspace. Please refer to the root [AGENTS.md](../../AGENTS.md) for global project guidelines and best practices.

## Crate-Specific Instructions

- **Location:** `packages/model-registry`
- **Package:** `model-registry`
- **Build:** Run `cargo build -p model-registry`
- **Test:** Run `cargo test -p model-registry`

## Key Facts

- 5 modules: categories, fetcher, info, policy, registry
- 5-dimension model classification: Capability, Tier, Cost, Context, Provider
- 6 modality types: Text, Image, Audio, Video, Embedding, Code
- 20+ providers in ProviderCategory enum
- Routing policy system with filters, actions, and conditions
- Leaf package — no internal workspace dependencies

## Known Pitfalls

- `Registry::get()` requires model ID to be non-empty (returns `Error::InvalidModelId`)
- Cache TTL is 1 hour by default — clones may serve stale data within that window
- Policy validation uses JSON schema at `config/policies.schema.json`

## Error Types

```rust
pub enum Error {
    ModelNotFound(String),  // No model matches the given ID
    Policy(String),         // Policy validation or application failure
    InvalidModelId(String), // Empty or malformed model ID
}
```
