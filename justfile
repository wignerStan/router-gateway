# Justfile for TypeScript Workspace Projects
# Production-ready task runner for comprehensive development operations
# Aligned with rust_v1.0 standards

set shell := ["bash", "-c"]

# ============================================
# Setup & Install
# ============================================

# Install all dependencies
install:
    bun install
    pre-commit install --hook-type pre-push --hook-type commit-msg

# Install development tools
install-dev:
    bun install
    bun install -d @types/node typescript tsx

# ============================================
# Workspace Operations
# ============================================

# List all workspace members
members:
    bun pm ls

# Show dependency graph
graph:
    echo "Dependency graph for workspace:"
    echo "bin --> api --> core"
    echo "frontend --> api --> core"
    echo "backend  --> api --> core"

# Show workspace structure
structure:
    echo "Workspace structure:"
    find packages -name "package.json" 2>/dev/null || echo "No packages found"

# Update all dependencies
update:
    bun update

# Check for outdated dependencies
outdated:
    bun pm outdated

# Show dependency tree
tree:
    echo "Use: bun pm ls to view dependencies"

# ============================================
# Development Tasks
# ============================================

# Build all packages
build:
    bun run build

# Build with watch mode
build-watch:
    bun run --filter '*' dev

# Run the app (bin package)
start:
    cd packages/bin && bun run start

# Run in development mode with watch
dev:
    cd packages/bin && bun run dev

# ============================================
# Testing Tasks
# ============================================

# Run all tests
test:
    bun run test

# Run tests with watch mode
test-watch:
    bun run --filter '*' test:watch

# Run tests with coverage
test-coverage:
    bun run --filter '*' test:coverage

# Run tests with verbose output
test-verbose:
    bun test --verbose

# ============================================
# Quality Tasks
# ============================================

# Format all code
fmt:
    prettier --write "**/*.{ts,js,json,md,yml,yaml}"

# Check formatting
fmt-check:
    prettier --check "**/*.{ts,js,json,md,yml,yaml}"

# Lint all code
lint:
    bun run lint

# Lint and fix
lint-fix:
    bun run --filter '*' lint:fix

# Type check
type-check:
    bun run type-check

# All quality checks
qa:
    just fmt && just lint && just type-check && just test

# Full QA suite
qa-full:
    just fmt && just lint && just type-check && just test && just security-scan && just audit

# ============================================
# Security Tasks
# ============================================

# Security scan (gitleaks)
security-scan:
    gitleaks detect --source . --verbose --report-path gitleaks-report.json

# Audit dependencies
audit:
    bun pm audit

# Check for unused dependencies
check-unused:
    echo "Manual check: review package.json dependencies vs imports"

# ============================================
# CI/CD Tasks
# ============================================

# Full CI pipeline
ci-full:
    just ci-lint && just ci-type-check && just ci-test && just ci-build

# CI lint
ci-lint:
    bun run lint

# CI type check
ci-type-check:
    bun run type-check

# CI test
ci-test:
    bun run test

# CI build
ci-build:
    bun run build

# CI format
ci-fmt:
    prettier --check "**/*.{ts,js,json,md,yml,yaml}"

# ============================================
# Pre-push Checks
# ============================================

# Pre-push validation
pre-push:
    bun run lint
    bun run type-check
    bun run test
    bun run build

# ============================================
# Clean Tasks
# ============================================

# Clean build artifacts
clean:
    rm -rf packages/*/dist

# Clean all artifacts
clean-all:
    rm -rf packages/*/dist
    rm -rf packages/*/node_modules
    rm -rf node_modules
    rm -rf bun.lockb
    rm -rf coverage

# ============================================
# Environment Tasks
# ============================================

# Set development environment
dev-env:
    export NODE_ENV=development
    export LOG_LEVEL=debug

# Set production environment
prod-env:
    export NODE_ENV=production
    export LOG_LEVEL=info

# Show environment
env:
    echo "NODE_ENV=${NODE_ENV:-not set}"
    echo "LOG_LEVEL=${LOG_LEVEL:-not set}"

# ============================================
# Utility Tasks
# ============================================

# Environment check
env-check:
    bun --version
    node --version 2>/dev/null || echo "Node not available (Bun only)"
    just --version
    echo "Working directory: $(pwd)"

# Generate lockfile
lockfile:
    bun install --frozen-lockfile

# ============================================
# Help
# ============================================

# Show help
help:
    @just --list

# Default task
default:
    @just --list

# ============================================
# Notes
# ============================================

# All tasks are designed for Bun runtime
# Use `just <task> --help` to see task-specific help
# Tasks can be chained: `just fmt && lint && test`
# For more information, see: https://just.systems/man/en/
