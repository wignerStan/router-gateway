# AGENTS.md

## Project Overview

A **local LLM gateway** written in Rust for intelligent request routing. Routes LLM requests to optimal credentials based on health, latency, and success rate. Designed for local development and self-hosted deployments.

**Stack**: Rust 1.85+, Tokio, Axum, SQLite
**Focus**: Smart routing, model registry, LLM tracing

### Package Names

> **Note:** Directory names differ from Cargo package names:

| Directory           | Package Name  |
| ------------------- | ------------- |
| `packages/tracing/` | `llm-tracing` |
| `apps/cli/`         | `my-cli`      |

## Build and test commands

| Task        | Command                    |
| ----------- | -------------------------- |
| Build       | `cargo build`              |
| Test        | `cargo test --workspace`   |
| Run Gateway | `cargo run -p gateway`     |
| Lint        | `cargo clippy --workspace` |
| Format      | `cargo fmt`                |
| All Checks  | `just qa`                  |

## Code style guidelines

This project enforces production-level code style using `rustfmt` and `clippy`. Adhere to the following conventions:

### General Idioms & Style

- **Cosmetic Discipline**: Adhere to standard naming (`snake_case`, `CamelCase`, `UPPER_SNAKE_CASE`). Group imports hierarchically.
- **Borrowing over Cloning**: Prefer borrowing (`&T`) over `.clone()` to avoid unnecessary allocations.
- **Prevent Early Allocation**: Do not collect into `Vec` unless explicitly necessary for returning or async bounds.
- **Result over Panic**: Return `Result<T, E>`. Never `unwrap()` or `expect()` in production code. Use `thiserror` (lib) / `anyhow` (bin).
- **Type State Pattern**: Use Type State for complex state machines to guarantee compile-time correctness.
- **No Living Comments**: Don't write out-of-sync comments. Let code describe the _what_ and comments the _why_.
- **Match statements**: Make match statements exhaustive and avoid wildcard arms (`_`) whenever possible. Use explicit arms for maintainability.
- **Range checking**: Use `(start..=end).contains(&val)` instead of manual `>=` and `<=` checks.
- **Borrowing**: Prefer borrowing over cloning. Prevent early allocations.
- **Iterators**: Prefer `.iter()` and iterator combinators over manual `for` loops for zero-cost abstractions and better readability.
- **Passing by Value vs Reference**: Follow official guidelines for when to pass by value (e.g. `Copy` traits) vs by reference.

### Error Handling

- **Result over Panic**: Prefer returning `Result` and avoid `panic!`.
- **Avoid unwraps**: Do not use `unwrap()` or `expect()` in production code. Handle errors gracefully.
- **Error Types**: Use `thiserror` for library/crate level errors and reserve `anyhow` strictly for binaries/applications.
- **Error Bubbling**: Use the `?` operator to bubble errors up.

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
- **Snapshot Testing**: Use snapshot testing (e.g., `cargo insta`) for output validation where appropriate.
- **Test Errors**: Ensure unit tests exercise error conditions and not just the happy path.
- **Code Style Enforcement**: All code changes **must** pass formatting and linting rules. Before finalizing changes run `cargo fmt`, `cargo clippy --workspace --all-targets` to fix lint issues, and `just qa` to run all project quality gates.

## Security considerations

- No `unwrap()`/`expect()` in production — prevents DoS via panic.
- Use `constant_time_token_matches()` for all auth token comparisons (timing side-channel prevention).
- Use `partial_cmp().unwrap_or(Ordering::Equal)` for all float comparisons (NaN safety).
- SSRF protection blocks private/reserved IPs and IPv6-mapped/compatible addresses.
- See [SECURITY.md](SECURITY.md) for full security policy, deployment practices, and reporting procedures.

## Project details

For architecture, features, configuration, API endpoints, and key types, see [README.md](README.md).

## Known Pitfalls

- All async operations use Tokio — always `.await` on registry/selector methods
- `Registry::get()` requires model ID to be non-empty (returns error otherwise)
- Health manager clones have independent storage (not shared state)
- Trace collectors are not shared across clones (same pattern)
- SQLite store requires `bundled` feature for cross-platform builds
- Model registry cache TTL is 1 hour by default
- Floating-point metrics may contain NaN values — always use `partial_cmp().unwrap_or(Ordering::Equal)` instead of `.unwrap()`
- `constant_time_token_matches()` must be used for all auth token comparisons to prevent timing attacks

<!-- BEGIN BEADS INTEGRATION -->

### Issue Tracking with bd (beads)

**IMPORTANT**: This project uses **bd (beads)** for ALL issue tracking. Do NOT use markdown TODOs, task lists, or other tracking methods.

#### Why bd?

- Dependency-aware: Track blockers and relationships between issues
- Git-friendly: Dolt-powered version control with native sync
- Agent-optimized: JSON output, ready work detection, discovered-from links
- Prevents duplicate tracking systems and confusion

#### Quick Start

**Check for ready work:**

```bash
bd ready --json
```

**Create new issues:**

```bash
bd create "Issue title" --description="Detailed context" -t bug|feature|task -p 0-4 --json
bd create "Issue title" --description="What this issue is about" -p 1 --deps discovered-from:bd-123 --json
```

**Claim and update:**

```bash
bd update <id> --claim --json
bd update bd-42 --priority 1 --json
```

**Complete work:**

```bash
bd close bd-42 --reason "Completed" --json
```

#### Issue Types

- `bug` - Something broken
- `feature` - New functionality
- `task` - Work item (tests, docs, refactoring)
- `epic` - Large feature with subtasks
- `chore` - Maintenance (dependencies, tooling)

#### Priorities

- `0` - Critical (security, data loss, broken builds)
- `1` - High (major features, important bugs)
- `2` - Medium (default, nice-to-have)
- `3` - Low (polish, optimization)
- `4` - Backlog (future ideas)

#### Workflow for AI Agents

1. **Check ready work**: `bd ready` shows unblocked issues
2. **Claim your task atomically**: `bd update <id> --claim`
3. **Work on it**: Implement, test, document
4. **Discover new work?** Create linked issue:
   - `bd create "Found bug" --description="Details about what was found" -p 1 --deps discovered-from:<parent-id>`
5. **Complete**: `bd close <id> --reason "Done"`

#### Auto-Sync

bd automatically syncs via Dolt:

- Each write auto-commits to Dolt history
- Use `bd dolt push`/`bd dolt pull` for remote sync
- No manual export/import needed!

#### Important Rules

- ✅ Use bd for ALL task tracking
- ✅ Always use `--json` flag for programmatic use
- ✅ Link discovered work with `discovered-from` dependencies
- ✅ Check `bd ready` before asking "what should I work on?"
- ❌ Do NOT create markdown TODO lists
- ❌ Do NOT use external issue trackers
- ❌ Do NOT duplicate tracking systems

For more details, see README.md and docs/QUICKSTART.md.

<!-- END BEADS INTEGRATION -->

### Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until pushing succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **SYNC AND PUBLISH** - This is MANDATORY:
   Use the standard Git and bd sync commands to publish your work and ensure the working directory is clean.
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND published
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**

- Work is NOT complete until code is published
- NEVER stop before publishing - that leaves work stranded locally
- If publishing fails, resolve and retry until it succeeds
