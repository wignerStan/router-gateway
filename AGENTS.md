# AGENT INSTRUCTIONS

## Project Overview

A **local LLM gateway** written in Rust for intelligent request routing. Routes LLM requests to optimal credentials based on health, latency, and success rate. Designed for local development and self-hosted deployments.

**Stack**: Rust 1.85+ (edition 2024), Tokio, Axum, SQLite
**Focus**: Smart routing, model registry, LLM tracing
**Package**: Single crate — `gateway` (no workspace)

### Build and Test Commands

| Task              | Command                                                        |
| ----------------- | -------------------------------------------------------------- |
| Build             | `cargo build`                                                  |
| Test (all)        | `cargo nextest run`                                            |
| Test (unit only)  | `cargo nextest run --lib`                                      |
| Test (one test)   | `cargo nextest run -E 'test(test_name_pattern)'`               |
| Test (doctests)   | `cargo test --doc`                                             |
| Test (property)   | `cargo nextest run -E 'test(proptests)'`                       |
| Benchmarks        | `cargo bench`                                                  |
| Fuzz (SSRF)       | `cargo +nightly fuzz run ssrf_url_fuzz -- -max_total_time=60`  |
| Fuzz (Config)     | `cargo +nightly fuzz run config_parse_fuzz -- -max_total_time=60` |
| Fuzz (Token)      | `cargo +nightly fuzz run token_match_fuzz -- -max_total_time=60` |
| Fuzz (All)        | `just fuzz-all`                                                |
| Run Gateway       | `cargo run --bin gateway`                                      |
| Lint              | `cargo clippy --all-targets -- -D warnings`                    |
| Format            | `cargo fmt --all`                                              |
| Coverage (HTML)   | `cargo llvm-cov nextest --html --ignore-filename-regex "src/main\.rs\|src/bin/cli\.rs"` |
| Coverage gate     | `cargo llvm-cov --fail-under-lines 90 --ignore-filename-regex "src/main\.rs\|src/bin/cli\.rs"` |
| Snapshots review  | `cargo insta test` then `cargo insta review`                   |
| Quick Checks      | `just qa`                                                      |
| Full Checks       | `just qa-full`                                                 |
| Security Deep     | `just qa-security`                                             |
| Test (concurrency)  | `cargo test --test loom_rate_limiter && cargo test --test shuttle_concurrency` |
| Test (error paths)  | `cargo test --test concurrency_error_paths` |
| Miri (unsafe)       | `MIRIFLAGS="-Zmiri-disable-isolation" cargo +nightly miri test --lib` |
| TSAN (races)        | `RUSTFLAGS="-Zsanitizer=thread" cargo +nightly test --lib --target x86_64-unknown-linux-gnu -- -- --test-threads=1` |
| Mutation testing    | `cargo mutants --in-place` |

**Runtime Diagnostics** (not automated tests):

| Task              | Command                                                        |
| ----------------- | -------------------------------------------------------------- |
| Console (debug)   | `cargo run --features console --bin gateway`                   |

### Source Layout

```
src/
├── main.rs, lib.rs, config.rs, state.rs, routes.rs     # Gateway core
├── routing/          # Smart credential selection
│   ├── bandit/       # Thompson sampling (beta/gamma distributions)
│   ├── classification/  # Request classification (vision, tools, streaming)
│   ├── config/       # Routing config (time/quota-aware strategies)
│   ├── health/       # Health state machine (Healthy → Degraded → Unhealthy)
│   ├── router/       # Dispatch: strategy → credential selection
│   ├── selector/     # Ranking and candidate filtering
│   └── weight/       # Composite weight calculation (success, latency, health, load, priority)
├── registry/         # Model metadata and routing policies
│   ├── policy/       # Policy engine (filters, actions, matchers)
│   └── registry/     # Model info CRUD and lookup
├── tracing/          # Request/response observability (TraceSpan, collector, middleware)
├── providers/        # LLM provider adapters (OpenAI, Anthropic, Google)
└── utils/            # Security (SSRF, constant-time compare), env helpers
benches/              # Criterion benchmarks (routing, SSRF performance)
fuzz/                 # cargo-fuzz targets (SSRF, config parsing, token matching)
```

Integration tests in `tests/` are split by domain: `classification_integration.rs`, `health_integration.rs`, `config.rs`, `routes.rs`, `tracing_integration.rs`, `cli_integration.rs`, `loom_rate_limiter.rs`, `shuttle_concurrency.rs`, `concurrency_error_paths.rs`.

### Key Types

| Type                    | Location                          | Purpose                                  |
| ----------------------- | --------------------------------- | ---------------------------------------- |
| `AppState`              | `src/state.rs`                    | Shared Axum state (registry, config)     |
| `GatewayConfig`         | `src/config.rs`                   | Full gateway YAML config                 |
| `SmartRoutingConfig`    | `src/routing/config/mod.rs`       | Strategy + weight configuration          |
| `Router`                | `src/routing/router/mod.rs`       | Strategy dispatch + credential selection |
| `HealthManager`         | `src/routing/health/mod.rs`       | Credential health state machine          |
| `WeightCalculator`      | `src/routing/weight/calculator.rs`| Composite scoring (5 factors)            |
| `Registry`              | `src/registry/registry/mod.rs`    | Model info CRUD + lookup                 |
| `PolicyRegistry`        | `src/registry/policy/registry.rs` | Routing policy engine                    |
| `TraceSpan`             | `src/tracing/trace.rs`            | Request/response trace record            |

### Known Pitfalls

- All async operations use Tokio — always `.await` on registry/selector methods
- `Registry::get()` requires model ID to be non-empty (returns error otherwise)
- Health manager clones share the same underlying `Arc<RwLock>` storage. Updates through one clone are visible to all.
- Trace collector clones share the same `Arc<RwLock>` buffer. Traces recorded via one clone appear in all.
- SQLite store requires `bundled` feature for cross-platform builds
- Model registry cache TTL is 1 hour by default
- Floating-point metrics may contain NaN values — always use `partial_cmp().unwrap_or(Ordering::Equal)` instead of `.unwrap()`
- `constant_time_token_eq()` must be used for all auth token comparisons to prevent timing attacks
- Fuzzing requires nightly Rust (`rustup toolchain install nightly; cargo +nightly install cargo-fuzz`)
- Fuzz corpus is stored in `fuzz/corpus/` — do not commit large corpora to git
- Line coverage measures executed code, not interleaving coverage. 90%+ line coverage does NOT guarantee race-freedom. Concurrency correctness relies on loom (systematic), shuttle (randomized async), and TSAN.

### REFERENCE

For architecture diagrams, API endpoints, configuration format, and provider details, see [README.md](README.md).

## Code style guidelines

This project enforces production-level code style using `rustfmt` and `clippy`. Adhere to the following conventions:

### General Idioms & Style

- **Borrowing over Cloning**: Prefer borrowing (`&T`) over `.clone()`. Do not collect into `Vec` unless explicitly necessary.
- **Result over Panic**: Return `Result<T, E>`. Never `unwrap()` or `expect()` in production code. Use `thiserror` (lib) / `anyhow` (bin).
- **Type State Pattern**: Use Type State for complex state machines to guarantee compile-time correctness.
- **No Living Comments**: Comments explain the _why_, not the _what_. Don't write out-of-sync comments.
- **Match statements**: Exhaustive matches, avoid wildcard arms (`_`) for business-critical enums.
- **Range checking**: Use `(start..=end).contains(&val)` instead of manual `>=` and `<=` checks.
- **Iterators**: Prefer `.iter()` and iterator combinators over manual `for` loops.
- **Passing by Value vs Reference**: Follow official Rust guidelines (`Copy` types by value, others by reference).

### Error Handling

- **No unwrap/expect in production**: Denied by clippy. Use `unwrap_or()` / `unwrap_or_else()` for defaults, `?` for propagation. If truly unavoidable, use `#[allow(clippy::expect_used)]` with a justification comment.
- **Error Types**: `thiserror` for library errors, `anyhow` for binaries.
- **Test code**: `expect("reason")` is allowed in tests for better failure messages.

### Comments and Documentation

- **Context, Not Clutter**: Comments should explain the _why_, not the _what_. If code is complex, refactor instead of over-commenting. Don't write living comments when documentation is needed.
- **Living Documentation**: Treat tests as living documentation. Add test examples to your doc comments.
- **TODOs**: `TODO` comments should generally become tracked issues.

### Modules & Architecture

- **Modularity & Size**: Avoid large modules. Prefer adding new modules instead of growing existing ones. Target Rust modules under 500 LoC (excluding tests). If a file exceeds 800 LoC, extract functionality into a new module instead of extending the existing file unless there is a strong documented reason not to.
- **Locality**: When extracting code, move the related tests and module/type docs toward the new implementation so the invariants stay close to the code that owns them.
- **Helper Methods**: Do not create small helper methods that are referenced only once.
- **Type State Pattern**: Consider using the Type State pattern for complex state machines to guarantee correctness at compile time.
- **Dispatch**: Use static dispatch (`impl Trait` or `<T: Trait>`) by default. Use dynamic dispatch (`dyn Trait`) only when necessary for heterogeneous collections or compile-time performance trade-offs.

### Async/Tokio Conventions

- All async operations use Tokio. Always `.await` on registry/selector methods.
- Maintain clear boundaries between async and sync code.

## Testing instructions

- **Deep Equals**: Prefer deep equals comparisons whenever possible. Perform `assert_eq!()` on entire objects rather than individual fields. Use `pretty_assertions::assert_eq` for clearer diffs.
- **Environment**: Avoid mutating process environment in tests; prefer passing environment-derived flags or dependencies from above.
- **Async Tests**: Always mark async tests as `#[tokio::test]`.
- **Sleeping**: Avoid `std::thread::sleep` in async contexts; always use `tokio::time::sleep`.
- **Test Errors**: Ensure unit tests exercise error conditions and not just the happy path.
- **Test Runner**: Use `cargo nextest run` (not `cargo test`). CI uses the `ci` profile with retries.
- **Coverage**: Minimum 90% line coverage enforced in CI via `cargo llvm-cov`. Excludes `src/main.rs` and `src/bin/cli.rs`.
- **Snapshot Testing**: Use `insta` (`assert_yaml_snapshot!`) for structured output. Review with `cargo insta test` + `cargo insta review`.
- **Property-Based Testing**: Use `proptest` for numeric edge cases and security invariants. Float-heavy modules (bandit, weight) and security modules (ssrf, security, env) have proptest suites. Run with `cargo nextest run -E 'test(proptests)'`.
- **Fuzzing**: Use `cargo-fuzz` (nightly) for security-critical parsing. Targets: `ssrf_url_fuzz`, `config_parse_fuzz`, `token_match_fuzz`. Run with `just fuzz-all` or individual `just fuzz-ssrf` commands.
- **Benchmarking**: Use `criterion` for performance regression detection. Run with `cargo bench` or `just bench`.
- **Parameterized Testing**: Use `rstest` (`#[rstest]` + `#[case]`) for data-driven tests with multiple inputs.
- **Red/Edge Tests**: Failure-path and boundary-condition tests go in `mod red_edge { ... }` within the existing test module. Run with `just test-red`. Count with `just test-red-count`. Target: 30-40% of all tests should be red/edge. Integration test files that are entirely failure-path (`routes_input_validation`, `db_resilience`, `concurrency_error_paths`) wrap all tests in `mod red_edge`.

## Security considerations

- No `unwrap()` in production — denied by clippy. `expect()` only with `#[allow(clippy::expect_used)]` + justification comment.
- Use `constant_time_token_eq()` for all auth token comparisons (timing side-channel prevention).
- Use `partial_cmp().unwrap_or(Ordering::Equal)` for all float comparisons (NaN safety).
- SSRF protection blocks private/reserved IPs and IPv6-mapped/compatible addresses.
- See [SECURITY.md](SECURITY.md) for full security policy, deployment practices, and reporting procedures.

