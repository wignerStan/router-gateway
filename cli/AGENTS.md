# my-cli

This project is part of the workspace. Please refer to the root [AGENTS.md](../AGENTS.md) for global project guidelines and best practices.

## Crate-Specific Instructions

- **Location:** `cli`
- **Package:** `my-cli`
- **Build:** Run `cargo build -p my-cli`
- **Test:** Run `cargo test -p my-cli`

## Key Facts

- CLI management utility built with clap
- Commands: `health`, `models`, `validate`
- Output formats: text (default), json (`-f/--format`)
- Verbose mode: `-v/--verbose`
- Standalone — no internal workspace dependencies
