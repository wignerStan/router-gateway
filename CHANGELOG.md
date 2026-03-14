# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Comprehensive doc comments for all public modules, structs, and enums across all packages (`gateway`, `my-cli`, `model-registry`, `smart-routing`, `llm-tracing`).
- Detailed documentation for stateful components (`HealthManager`, `MetricsCollector`) with a focus on clone semantics and internal storage sharing.
- New integration tests for `llm-tracing` Axum middleware to verify trace collection during the HTTP lifecycle.
- Descriptive and context-aware `.expect()` messages in all test files to improve diagnostics.
- Local LLM gateway with smart request routing.
- Five routing strategies: weighted, time_aware, quota_aware, adaptive, policy_aware.
- OpenAI-compatible API at `POST /v1/chat/completions`.
- Three provider adapters: OpenAI, Google, Anthropic.
- Model registry with 5-dimension classification.
- LLM request tracing and observability.
- Management CLI (`my-cli`).
- SQLite-backed metrics and health persistence.
- SSRF protection for credential base URLs.
- Constant-time token comparison (timing attack prevention).
- BDD integration test suite.

### Changed

- Promoted `missing_docs` from `warn` to `deny` in the workspace `Cargo.toml`.
- Configured workspace-level `clippy::all` and `clippy::pedantic` as `deny`.
- Replaced `#[allow(clippy::panic)]` with idiomatic `assert!(matches!(...))` in several tests.
- Simplified `GatewayConfig::expand_env_vars` to return `()` as it no longer produces errors.

### Fixed

- Improved lock scoping in `MetricsCollector`, `HealthManager`, and `SQLiteStore` to address `significant_drop_tightening` and reduce contention.

[Unreleased]: https://github.com/wignerStan/router-gateway/compare/main...HEAD
