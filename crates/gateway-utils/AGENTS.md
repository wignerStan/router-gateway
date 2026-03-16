# gateway-utils

This project is part of the workspace. Please refer to the root [AGENTS.md](../../AGENTS.md) for global project guidelines and best practices.

## Crate-Specific Instructions

- **Location:** `crates/gateway-utils`
- **Package:** `gateway-utils`
- **Build:** Run `cargo build -p gateway-utils`
- **Test:** Run `cargo test -p gateway-utils`

## Key Facts

- Shared security and utility functions used by the gateway and CLI
- Timing-safe token comparison (constant-time equality)
- SSRF protection (private IP / loopback / link-local / metadata address blocking)
- Environment variable expansion (`${VAR}`, `${VAR:-default}`)
- Leaf package — no internal workspace dependencies
