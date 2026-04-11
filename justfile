# Justfile for Rust Workspace Projects
# Production-ready task runner with Tiered Verification System
# Aligned with Agentic Native principles

set shell := ["bash", "-c"]

# Workspace members for quick reference
members := "cli crates/gateway crates/smart-routing crates/model-registry crates/llm-tracing"

# ============================================
# DEFAULT & HELP
# ============================================

# Default: show available commands
default:
    @just --list

# Show help with categories
help:
    @echo "╔════════════════════════════════════════════════════════════╗"
    @echo "║              GATEWAY WORKSPACE - JUST COMMANDS              ║"
    @echo "╠════════════════════════════════════════════════════════════╣"
    @echo "║ TIER 1 (QUICK <3s): just qa                                ║"
    @echo "║ TIER 2 (LINT <10s): just qa-lint                           ║"
    @echo "║ TIER 3 (FULL >30s): just qa-full                           ║"
    @echo "╠════════════════════════════════════════════════════════════╣"
    @echo "║ DEV:     start, dev, cli, watch                            ║"
    @echo "║ BUILD:   build, build-release, docs                        ║"
    @echo "║ TEST:    test, test-package, test-coverage                 ║"
    @echo "║ QUALITY: fmt, lint, check, qa, qa-lint, qa-full            ║"
    @echo "║ SECURITY: audit, security-scan                             ║"
    @echo "║ UTILITY: members, graph, outdated, env                     ║"
    @echo "║ JQ:      jq-members, jq-deps, jq-features, jq-manifest     ║"
    @echo "╚════════════════════════════════════════════════════════════╝"

# ============================================
# TIERED VERIFICATION SYSTEM
# ============================================

# Tier 1: Quick feedback (<3s) - Use during development / pre-commit
qa: fmt-check check
    @echo "Tier 1 QA passed (quick checks)"

# Tier 2: Lint checks (<10s) - Use before push
qa-lint: qa lint
    @echo "Tier 2 QA passed (lint checks)"

# Tier 3: Full verification (>30s) - Use for CI / release
qa-full: qa-lint test security-audit
    @echo "Tier 3 QA passed (full verification)"

# ============================================
# SETUP & INSTALL
# ============================================

# Install all dependencies and setup hooks
install:
    cargo fetch
    lefthook install

# Install development tools
install-dev:
    cargo fetch
    rustup component add clippy rustfmt

# ============================================
# FAST DEVELOPMENT COMMANDS (<5s)
# ============================================

# Type check workspace (fast feedback)
type-check:
    cargo check --all

# Quick type check (quiet mode, instant feedback)
check:
    cargo check -q

# Format all code
fmt:
    cargo fmt --all

# Check formatting (fast)
fmt-check:
    cargo fmt --all -- --check

# Fast lint (warnings only, for development)
lint-fast:
    cargo clippy --all-targets

# Strict lint (treat warnings as errors, for pre-push)
lint:
    cargo clippy --all-targets -- -D warnings

# Run clippy with auto-fix
lint-fix:
    cargo clippy --all-targets --all-features --fix --allow-dirty

# ============================================
# SLOW DEVELOPMENT COMMANDS (>5s)
# ============================================

# Build all packages (debug)
build:
    cargo build

# Build all packages (release)
build-release:
    cargo build --release

# Build specific package
build-package PACKAGE:
    cargo build -p {{PACKAGE}}

# Build with watch mode
watch:
    cargo watch -x "check --all" -x "test --all"

# Build and watch
build-watch:
    cargo watch -x build

# ============================================
# TESTING
# ============================================

# Run all tests
test:
    cargo test --all

# Run tests with verbose output
test-verbose:
    cargo test --all -- --nocapture

# Fast unit tests only (skip integration tests)
test-fast:
    cargo test --all --lib

# Run tests with coverage (requires cargo-tarpaulin)
test-coverage:
    cargo tarpaulin --all --out Xml --output-dir coverage

# Run tests for specific package
test-package PACKAGE:
    cargo test -p {{PACKAGE}}

# Run doc tests
test-doc:
    cargo test --doc --all

# Run specific test by name pattern
test-filter PATTERN:
    cargo test --all {{PATTERN}}

# Run smart-routing tests
test-routing:
    cargo test -p smart-routing

# Run model-registry tests
test-registry:
    cargo test -p model-registry

# Run tracing tests
test-tracing:
    cargo test -p llm-tracing

# Run gateway tests
test-gateway:
    cargo test -p gateway

# ============================================
# RUNNING APPLICATIONS
# ============================================

# Run the gateway app
start:
    cargo run --bin gateway

# Run in development mode with debug logging
dev:
    RUST_LOG=debug RUST_BACKTRACE=1 cargo run --bin gateway

# Run CLI tool
cli *ARGS:
    cargo run --bin cli -- {{ARGS}}

# Run gateway with specific config
run-config CONFIG:
    cargo run --bin gateway -- --config {{CONFIG}}

# ============================================
# SECURITY
# ============================================

# Audit dependencies for vulnerabilities
audit:
    cargo audit

# Security scan (gitleaks)
security-scan:
    gitleaks detect --source . --verbose --report-path gitleaks-report.json

# Combined security check
security-audit: audit security-scan
    @echo "✅ Security audit complete"

# Check for security issues only
security:
    just security-audit

# ============================================
# DOCUMENTATION
# ============================================

# Build documentation
docs:
    cargo doc --no-deps --all

# Open documentation in browser
docs-open:
    cargo doc --no-deps --all --open

# Build docs for specific package
docs-package PACKAGE:
    cargo doc --no-deps -p {{PACKAGE}}

# ============================================
# JQ COMMANDS FOR DEVELOPMENT
# ============================================

# List all workspace members (json output)
jq-members:
    cargo metadata --no-deps --format-version 1 | jq '.packages[] | {name, version, manifest_path}'

# Show dependency tree as JSON
jq-deps:
    cargo metadata --format-version 1 | jq '.resolve.nodes[] | {id, deps: [.deps[].name]}'

# List all features available in workspace
jq-features:
    cargo metadata --format-version 1 | jq '[.packages[] | {name, features: (.features | keys)}]'

# Show manifest summary
jq-manifest:
    cargo metadata --no-deps --format-version 1 | jq '{workspace_members: [.packages[].name], total_packages: (.packages | length)}'

# Find unused dependencies
jq-unused:
    cargo metadata --format-version 1 | jq '.packages[] | {name, dependencies: [.dependencies[].name]}'

# Show all dependencies with versions
jq-deps-versions:
    cargo metadata --format-version 1 | jq '.packages[] | select(.id | startswith("registry+")) | {name, version}' | sort -u

# Get package info by name
jq-package NAME:
    cargo metadata --no-deps --format-version 1 | jq '.packages[] | select(.name == "{{NAME}}")'

# Show compilation targets
jq-targets:
    cargo metadata --no-deps --format-version 1 | jq '.packages[] | {name, targets: [.targets[].name]}'

# ============================================
# WORKSPACE OPERATIONS
# ============================================

# List all workspace members
members:
    @echo "📦 Workspace Members:"
    @cargo metadata --no-deps --format-version 1 | jq -r '.packages[].name' | sed 's/^/  /'

# Show dependency graph
graph:
    cargo tree --duplicates || cargo tree

# Show workspace structure
structure:
    @echo "📁 Workspace Structure:"
    @find crates cli -name "Cargo.toml" 2>/dev/null | head -20

# Update all dependencies
update:
    cargo update

# Check for outdated dependencies
outdated:
    cargo outdated 2>/dev/null || cargo update --dry-run

# Check for unused dependencies
check-unused:
    cargo machete 2>/dev/null || cargo tree --duplicates

# Generate lockfile
lockfile:
    cargo generate-lockfile

# ============================================
# PRE-COMMIT / PRE-PUSH HOOKS
# ============================================

# Pre-commit: quick checks (matches lefthook)
pre-commit: fmt-check check
    @echo "Pre-commit checks passed"

# Pre-push: lint checks (matches lefthook)
pre-push: qa-lint
    @echo "Pre-push checks passed"

# ============================================
# CI/CD TASKS
# ============================================

# Full CI pipeline
ci-full: ci-fmt ci-lint ci-test ci-build
    @echo "✅ CI pipeline complete"

# CI format check
ci-fmt:
    cargo fmt --all -- --check

# CI lint (strict)
ci-lint:
    cargo clippy --all-targets -- -D warnings

# CI test (all features)
ci-test:
    cargo test --all --all-features

# CI build (release)
ci-build:
    cargo build --release

# CI type check
ci-type-check:
    cargo check --all --all-features

# CI workspace lint inheritance check
ci-lint-inheritance:
    ./scripts/check-lint-inheritance.sh

# ============================================
# CLEAN TASKS
# ============================================

# Clean build artifacts
clean:
    cargo clean

# Clean target directory for specific package
clean-package PACKAGE:
    rm -rf target/debug/deps/{{PACKAGE}}*
    rm -rf target/release/deps/{{PACKAGE}}*

# Clean generated docs
clean-docs:
    rm -rf target/doc

# ============================================
# ENVIRONMENT
# ============================================

# Show environment info
env:
    @echo "🔧 Environment:"
    @echo "  RUST_LOG: ${RUST_LOG:-not set}"
    @echo "  RUST_BACKTRACE: ${RUST_BACKTRACE:-not set}"
    @echo "  RUSTFLAGS: ${RUSTFLAGS:-not set}"

# Environment check
env-check:
    @rustc --version
    @cargo --version
    @rustup --version 2>/dev/null || echo "rustup not available"
    @just --version

# ============================================
# UTILITY
# ============================================

# Run specific example
run-example EXAMPLE:
    cargo run --example {{EXAMPLE}}

# Check compile times
check-time:
    cargo clean
    time cargo build --release

# Show binary sizes
binary-sizes:
    @echo "📊 Binary Sizes:"
    @ls -lh target/debug/{gateway,my-cli} 2>/dev/null || echo "Build first with: just build"

# Quick status check
status: members env
    @echo ""
    @echo "📊 Build Status:"
    @cargo check --all 2>&1 | tail -5

# ============================================
# NOTES
# ============================================

# Tiered Verification:
#   Tier 1 (Quick <3s):  just qa           -> fmt-check, check
#   Tier 2 (Lint <10s):  just qa-lint      -> qa, lint
#   Tier 3 (Full >30s):  just qa-full      -> qa-lint, test, security-audit
#
# Quick Development Flow:
#   just qa              # Quick feedback during development
#   just qa-lint         # Lint check before push
#   just test            # Run tests
#   just qa-full         # Full verification before release
#
# Git Hooks (lefthook):
#   pre-commit: fmt-check + cargo check -q
#   pre-push:   clippy -- -D warnings
#
# JQ Commands for Analysis:
#   just jq-members      # List packages as JSON
#   just jq-deps         # Dependency tree as JSON
#   just jq-features     # All features available
#   just jq-package core # Get specific package info
#
# For more information: https://just.systems/man/en/
