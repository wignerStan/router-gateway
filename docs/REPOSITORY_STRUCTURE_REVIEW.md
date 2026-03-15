# Repository Structure Review: Gateway Rust Workspace

**Date:** 2026-02-17
**Reviewer:** executor-2-structure
**Framework:** Agentic Native Principles

---

## Executive Summary

The Gateway repository follows many **Agentic Native** best practices but has several opportunities for improvement to maximize AI agent navigability and developer experience.

**Overall Score: 7.5/10**

---

## Current Structure Analysis

```
gateway/
├── apps/                      # Deployable binaries
│   ├── cli/                   # CLI tool (placeholder)
│   └── gateway/               # Main HTTP gateway server
├── packages/                  # Shared libraries
│   ├── core/                  # Shared utilities (placeholder)
│   ├── model-registry/        # Model classification & registry
│   ├── smart-routing/         # Routing logic with SQLite persistence
│   └── tracing/               # Observability & metrics
├── docs/                      # Documentation
├── .agents/                   # Agent context & skills
├── Cargo.toml                 # Workspace root
└── justfile                   # Unified command interface
```

---

## Compliance with Agentic Native Principles

### 1. Unified Monorepo Structure (apps/packages) - PASS

**Status:** Excellent

- Clear separation of `apps/` (deployables) and `packages/` (libraries)
- Workspace configuration properly defines all members
- No language silos (pure Rust workspace)

### 2. Feature-First Organization - PARTIAL

**Status:** Needs Improvement

**Findings:**

- `packages/smart-routing/` has good module organization:
  - `router.rs`, `selector.rs`, `health.rs`, `metrics.rs`, `weight.rs`
  - SQLite persistence isolated in `sqlite/` subdirectory
- `packages/model-registry/` is well-structured:
  - `categories.rs`, `registry.rs`, `info.rs`, `fetcher.rs`
- `packages/tracing/` follows similar patterns

**Issue:** The `apps/gateway/src/main.rs` contains inline route handlers (root, health_check, list_models, route_request) instead of feature modules.

**Recommendation:**

```rust
// apps/gateway/src/
//   main.rs          # Bootstrap only
//   routes/
//     mod.rs
//     health.rs      # Health check endpoint
//     models.rs      # Model listing endpoint
//     routing.rs     # Routing endpoint
//   state.rs         # AppState definition
```

### 3. The Triad (Code + README + Tests) - NEEDS IMPROVEMENT

**Status:** Critical Gap

**Missing Elements:**

- No `README.md` in any package (`packages/*/README.md` - MISSING)
- No `README.md` in any app (`apps/*/README.md` - MISSING)
- Tests exist but are scattered (inline `#[cfg(test)]` modules, `sqlite/tests.rs`)

**Recommendation:** Create local READMEs for each package:

```markdown
# packages/model-registry/README.md (proposed)

## Purpose

Model classification and registry for LLM providers.

## Key Components

- `categories.rs` - Tier, capability, and cost categorization
- `registry.rs` - Central model registry with trait-based fetchers
- `info.rs` - ModelInfo, ModelCapabilities, RateLimits types
- `fetcher.rs` - StaticFetcher trait implementation

## Dependencies

- async-trait, chrono, serde, thiserror, tokio

## Usage

use model_registry::{Registry, ModelInfo, ModelCategorization};
```

### 4. Unified Command Interface (justfile) - EXCELLENT

**Status:** Fully Compliant

The `justfile` provides comprehensive task coverage:

- Development: `just dev`, `just build`, `just start`
- Quality: `just fmt`, `just lint`, `just qa`, `just qa-full`
- Security: `just audit`, `just security-scan`
- CI/CD: `just ci-full`, `just pre-push`
- Workspace: `just members`, `just graph`, `just structure`

**Minor Enhancement Suggestion:** Add package-specific tasks:

```just
# Run smart-routing tests
test-routing:
    cargo test -p smart-routing

# Run model-registry tests
test-registry:
    cargo test -p model-registry
```

### 5. Documentation Strategy - PARTIAL

**Status:** Present but Incomplete

**Current State:**

- Root `docs/` exists with:
  - `API_TRANSFORMATION.md`
  - `MODEL_CLASSIFICATION.md`
- `.agents/skills/` contains agent-specific skills

**Missing per Diátaxis Framework:**

- `docs/tutorials/` - Learning-oriented guides
- `docs/guides/` - How-to guides
- `docs/reference/` - API/CLI reference
- `docs/explanations/` - Architecture decisions

**Recommendation:**

```
docs/
├── tutorials/
│   └── getting-started.md
├── guides/
│   ├── adding-new-provider.md
│   └── custom-routing-rules.md
├── reference/
│   ├── api.md            # OpenAPI spec reference
│   └── configuration.md
├── explanations/
│   ├── architecture.md
│   └── adr/              # Architecture Decision Records
│       └── ADR-001-sqlite-persistence.md
└── existing files...
```

### 6. Test Organization - NEEDS IMPROVEMENT

**Status:** Partial Compliance

**Current State:**

- Unit tests: Inline `#[cfg(test)]` modules (acceptable for Rust)
- Integration tests: `apps/gateway/src/main.rs` contains `integration_tests` module

**Issues:**

1. No dedicated `tests/` directory at workspace root for E2E tests
2. No BDD-style specification tests
3. Test fixtures are not centralized

**Recommendation:**

```
tests/
├── e2e/
│   └── gateway_flow.rs    # Full system tests
├── fixtures/
│   └── models.json        # Test data
└── common/
    └── mod.rs             # Shared test utilities
```

### 7. Type and Schema Organization - GOOD

**Status:** Compliant

- Domain types are co-located with their modules
- `model-registry/info.rs` contains ModelInfo types
- `smart-routing/config.rs` contains routing configuration types
- No unnecessary shared schema package needed (appropriate for current scale)

---

## Rust-Specific Best Practices Review

### Workspace Configuration

**Cargo.toml (root):**

```toml
[workspace]
resolver = "2"
members = [
    "apps/cli",
    "apps/gateway",
    "packages/smart-routing",
    "packages/model-registry",
    "packages/tracing",
    "packages/core"
]
```

**Recommendations:**

1. **Add workspace-level metadata:**

```toml
[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/org/gateway"

[workspace.dependencies]
# Shared dependency versions
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.40", features = ["sync", "time", "rt", "rt-multi-thread", "macros"] }
thiserror = "1.0"
```

2. **Add `[workspace.lints]` for consistency:**

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[workspace.lints.clippy]
all = "warn"
pedantic = "warn"
```

### Package Structure Issues

**packages/core/src/lib.rs** contains only a placeholder `Greeting` struct:

- This package appears unused by other packages
- Consider either removing or populating with actual shared utilities

**apps/cli/src/main.rs** is a placeholder that only uses `my_core::Greeting`:

- Should be removed or clearly marked as a template/placeholder

---

## Priority Action Items

### High Priority (Do First)

1. **Create package READMEs** - Add `README.md` to each package under `packages/` and `apps/`
2. **Refactor gateway routes** - Extract route handlers from `main.rs` into feature modules
3. **Add workspace dependencies** - Consolidate version management in root `Cargo.toml`

### Medium Priority

4. **Reorganize documentation** - Implement Diátaxis framework structure
5. **Create E2E test directory** - Add `tests/e2e/` for integration testing
6. **Add ADR directory** - Document architectural decisions

### Low Priority (Nice to Have)

7. **Remove or develop placeholder packages** - `packages/core` and `apps/cli`
8. **Add package-specific justfile tasks**
9. **Generate API documentation** - Configure `cargo doc` automation

---

## Summary Matrix

| Principle                     | Status  | Score |
| ----------------------------- | ------- | ----- |
| Unified Monorepo              | PASS    | 9/10  |
| Feature-First Organization    | PARTIAL | 7/10  |
| The Triad (README+Code+Tests) | FAIL    | 4/10  |
| Unified Commands (justfile)   | PASS    | 9/10  |
| Documentation Strategy        | PARTIAL | 6/10  |
| Test Organization             | PARTIAL | 6/10  |
| Type Organization             | PASS    | 8/10  |
| Rust Workspace Best Practices | PARTIAL | 7/10  |

**Overall: 7.5/10**

---

## Next Steps

1. Review this document with the team
2. Prioritize High Priority items for immediate implementation
3. Create follow-up tasks for Medium and Low priority items

---

_Generated by executor-2-structure as part of gateway-docs-team_
