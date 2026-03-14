# Contributing to Gateway

This repository is optimized for both human developers and autonomous AI coding agents. We follow strict quality gates and a plan-first development workflow.

## Prerequisites

- [Rust 1.85+](https://rustup.rs/) — use `rustup` for installation and management
- [Just](https://github.com/casey/just) — task runner (`cargo install just`)

## Quick Start

```bash
git clone <repo-url> && cd gateway
cargo build
cargo test --workspace
just qa
```

## Development Workflow

We follow the protocols defined in [AGENTS.md](./AGENTS.md):

1. **Check for work**: `bd ready` shows unblocked issues
2. **Plan First**: Never code without a tracked task
3. **Implement**: Write code following project conventions
4. **QA**: Run `just qa` before every commit

## Quality Gates

Before submitting a PR, ensure all checks pass:

```bash
just qa
```

This runs:

- **Format**: `rustfmt` style check
- **Lint**: `clippy` with pedantic and perf lints (deny)
- **Type-check**: `cargo check` with all features
- **Tests**: `cargo test --workspace`

For comprehensive checks: `just qa-full`

## Commit Guidelines

We use [Conventional Commits](https://www.conventionalcommits.org/):

- `feat`: New feature
- `fix`: Bug fix
- `refactor`: Code restructuring
- `docs`: Documentation changes
- `test`: Adding/modifying tests
- `chore`: Maintenance (dependencies, tooling)

## Code Style

Enforced by `rustfmt` and `clippy` — no manual formatting needed. See [AGENTS.md](./AGENTS.md) for project-specific conventions.

## Issue Tracking

All issues are tracked with `bd` (beads). See [AGENTS.md](./AGENTS.md) for the full workflow.
