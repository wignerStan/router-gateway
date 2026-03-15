# tests

This directory contains BDD integration tests. Please refer to the root [AGENTS.md](../AGENTS.md) for global project guidelines and best practices.

## Overview

- `bdd_integration_tests.rs` — BDD scenarios testing health management and request classification (vision, tools, streaming, thinking, context size)
- Run with: `cargo test --test bdd_integration_tests`

## Known Pitfalls

- Tests use in-memory configuration — no external config file is required
- Integration tests are slower than unit tests; run targeted tests with `cargo test --test bdd_integration_tests <filter>`
