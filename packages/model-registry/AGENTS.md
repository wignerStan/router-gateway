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
