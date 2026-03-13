# AGENTS.md

Local LLM gateway in Rust. Routes requests to optimal credentials based on health, latency, and success rate.

**Stack**: Rust 1.85+, Tokio, Axum, SQLite

> For architecture, features, configuration, and API details, see [README.md](README.md).
> For security policy and vulnerability reporting, see [SECURITY.md](SECURITY.md).

## Package Names

> Directory names differ from Cargo package names:

| Directory           | Package Name  |
| ------------------- | ------------- |
| `packages/tracing/` | `llm-tracing` |
| `apps/cli/`         | `my-cli`      |

## Commands

| Task        | Command                    |
| ----------- | -------------------------- |
| Build       | `cargo build`              |
| Test        | `cargo test --workspace`   |
| Run Gateway | `cargo run -p gateway`     |
| Lint        | `cargo clippy --workspace` |
| Format      | `cargo fmt`                |
| All Checks  | `just qa`                  |

## Code Style

Production-level style enforced via `rustfmt` and `clippy`.

### General Idioms

- **Borrowing over Cloning**: Prefer `&T` over `.clone()`. Prevent early allocations — don't collect into `Vec` unless necessary.
- **Result over Panic**: Return `Result<T, E>`. Never `unwrap()` or `expect()` in production. Use `thiserror` (lib) / `anyhow` (bin).
- **Type State Pattern**: Use for complex state machines to guarantee compile-time correctness.
- **Match statements**: Exhaustive, avoid wildcard `_` arms.
- **Range checking**: Use `(start..=end).contains(&val)`.
- **Iterators**: Prefer `.iter()` and combinators over manual `for` loops.

### Error Handling

- `thiserror` for library/crate errors, `anyhow` strictly for binaries.
- Bubble errors with `?`.
- No `unwrap()`/`expect()` in production code.

### Documentation

- Comments explain _why_, not _what_. Refactor complex code instead of over-commenting.
- Tests are living documentation. `TODO` comments should become tracked issues.

### Modules & Architecture

- Target modules under 500 LoC (excluding tests). Extract at 800 LoC.
- Keep related types, impls, and tests close together (locality).
- Static dispatch by default. `dyn Trait` only for heterogeneous collections.
- All async uses Tokio. Maintain clear async/sync boundaries.

## Testing

- **Deep equals**: `assert_eq!()` on entire objects. Use `pretty_assertions`.
- **Async tests**: Always `#[tokio::test]`.
- **Sleeping**: `tokio::time::sleep` only, never `std::thread::sleep` in async contexts.
- **Environment**: Don't mutate process env in tests; pass from above.
- **Test errors**: Exercise error conditions, not just happy path.
- **Style enforcement**: All changes must pass `cargo fmt`, `cargo clippy --workspace --all-targets`, and `just qa`.

## Security

- No `unwrap()`/`expect()` — prevents DoS via panic.
- Use `constant_time_token_matches()` for all auth token comparisons (timing side-channel prevention).
- Use `partial_cmp().unwrap_or(Ordering::Equal)` for all float comparisons (NaN safety).
- SSRF protection blocks private/reserved IPs and IPv6-mapped/compatible addresses.

> See [SECURITY.md](SECURITY.md) for full security policy, deployment practices, and reporting procedures.

## Key Pitfalls

- All async operations use Tokio — always `.await` on registry/selector methods.
- `Registry::get()` requires non-empty model ID (returns error otherwise).
- Health manager and trace collector clones have independent storage — not shared state.
- SQLite store requires `bundled` feature for cross-platform builds.
- Model registry cache TTL defaults to 1 hour.

## Reference

- Architecture & Features: [README.md](README.md)
- Security Policy: [SECURITY.md](SECURITY.md)
- Model Classification: `docs/MODEL_CLASSIFICATION.md`
- API Transformation: `docs/API_TRANSFORMATION.md`
- Quick Start: `docs/QUICKSTART.md`

<!-- BEGIN BEADS INTEGRATION -->

## Issue Tracking with bd (beads)

**IMPORTANT**: This project uses **bd (beads)** for ALL issue tracking. Do NOT use markdown TODOs, task lists, or other tracking methods.

### Quick Start

```bash
bd ready --json                          # Find unblocked work
bd create "Title" --description="..." -t feature -p 1 --json
bd update <id> --claim --json            # Claim a task
bd close <id> --reason "Done" --json     # Complete work
```

### Rules

- Use bd for ALL task tracking — no markdown TODO lists, no external trackers
- Always use `--json` flag for programmatic use
- Link discovered work with `--deps discovered-from:<parent-id>`
- Priorities: `0` (critical) → `4` (backlog). Types: `bug`, `feature`, `task`, `epic`, `chore`

<!-- END BEADS INTEGRATION -->

## Session Completion

**MANDATORY** before saying "done":

1. File beads issues for remaining work
2. Run quality gates (`just qa`) if code changed
3. Update beads issue status — close finished work
4. **Push** — work is NOT complete until published
5. Clean up stashes and prune remote branches
6. Hand off context for next session
