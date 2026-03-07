# my-cli

Command-line interface for the Gateway management.

## Purpose

CLI utility for gateway administration, health monitoring, and configuration management.

## Features

- **Health checks**: `my-cli health --url http://localhost:3000`
- **Model listing**: `my-cli models --url http://localhost:3000`
- **Configuration validation**: `my-cli validate --config config.toml`

## Usage

```bash
# Check gateway health
cargo run -p my-cli -- health

# List available models
cargo run -p my-cli -- models

# Validate configuration
cargo run -p my-cli -- validate --config /path/to/config.toml
```

## Dependencies

- `clap` - CLI argument parsing with derive macros
- `anyhow` - Error handling
- `tracing` / `tracing-subscriber` - Logging

## Tests

```bash
cargo test -p my-cli
```
