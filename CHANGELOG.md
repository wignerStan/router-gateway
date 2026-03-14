# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Local LLM gateway with smart request routing
- Five routing strategies: weighted, time_aware, quota_aware, adaptive, policy_aware
- OpenAI-compatible API at `POST /v1/chat/completions`
- Three provider adapters: OpenAI, Google, Anthropic
- Model registry with 5-dimension classification
- LLM request tracing and observability
- Management CLI (`my-cli`)
- SQLite-backed metrics and health persistence
- SSRF protection for credential base URLs
- Constant-time token comparison (timing attack prevention)
- BDD integration test suite
