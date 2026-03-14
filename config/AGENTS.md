# config

This directory contains shared configuration files. Please refer to the root [AGENTS.md](../AGENTS.md) for global project guidelines and best practices.

## Overview

- `policies.json` — Routing policy definitions (filters, actions, conditions)
- `policies.schema.json` — JSON Schema for validating routing policies
- `gateway.example.yaml` — Template configuration file for the gateway

## Known Pitfalls

- `policies.schema.json` is the source of truth for policy validation — the model-registry crate validates against this schema at runtime
- `policies.json` must conform to the schema or the gateway will fail to start
- Environment variable expansion (`${VAR}` and `${VAR:-default}`) happens at config load time in the gateway app, not in these files
