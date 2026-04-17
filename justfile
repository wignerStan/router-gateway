# Justfile for Rust Gateway Project
# Production-ready task runner with Tiered Verification System

set shell := ["bash", "-c"]

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
    @echo "║ TIER 4 (SECURITY):  just qa-security                       ║"
    @echo "║ TIER 5 (MUTATION): just qa-mutation                        ║"
    @echo "╠════════════════════════════════════════════════════════════╣"
    @echo "║ DEV:     start, dev, cli, watch, console                   ║"
    @echo "║ BUILD:   build, build-release, docs                        ║"
    @echo "║ TEST:    test, test-package, test-coverage, test-snapshots  ║"
    @echo "║ BENCH:   bench, bench-target, bench-save                   ║"
    @echo "║ FUZZ:    fuzz-ssrf, fuzz-config, fuzz-token, fuzz-all      ║"
    @echo "║ ADVANCED: miri, tsan, loom, shuttle, mutants               ║"
    @echo "║ QUALITY: fmt, lint, check, qa, qa-lint, qa-bdd, qa-full    ║"
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

# Tier 2.5: BDD + lint (<20s) - Pre-push quality gate
qa-bdd: qa-lint bdd
    @echo "Tier 2.5 QA passed (lint + BDD checks)"

# Tier 3: Full verification (>30s) - Use for CI / release
qa-full: qa-lint test test-coverage-check loom shuttle security-audit
    @echo "Tier 3 QA passed (full verification)"

# Tier 4: Security deep (property tests + miri + tsan + audit) - Use for releases
qa-security: test test-coverage-check
    cargo nextest run -E 'test(proptests)'
    MIRIFLAGS="-Zmiri-disable-isolation" cargo +nightly miri test --lib -E 'test(env)'
    RUSTFLAGS="-Zsanitizer=thread" TSAN_OPTIONS="halt_on_error=1" cargo +nightly test --lib --target x86_64-unknown-linux-gnu -- --test-threads=1 || true
    cargo audit
    @echo "Tier 4 QA passed (security deep)"

# Tier 5: Mutation testing - Use for release candidates
qa-mutation: qa-full mutants-check
    @echo "Tier 5 QA passed (mutation testing)"

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
    rustup component add clippy rustfmt llvm-tools-preview
    cargo install cargo-nextest cargo-llvm-cov cargo-insta

# Install all development tools including advanced testing
install-dev-all: install-dev install-fuzz
    cargo install cargo-mutants tokio-console
    rustup component add miri --toolchain nightly
    rustup component add rust-src --toolchain nightly

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

# Build specific binary
build-bin BIN:
    cargo build --bin {{BIN}}

# Build with watch mode
watch:
    cargo watch -x "check --all" -x "test --all"

# Build and watch
build-watch:
    cargo watch -x build

# ============================================
# TESTING
# ============================================

# Run all tests (uses nextest for faster execution)
test:
    cargo nextest run

# Run tests with verbose output
test-verbose:
    cargo nextest run --success-output immediate

# Fast unit tests only (skip integration tests)
test-fast:
    cargo nextest run --lib

# Run tests with coverage (requires cargo-llvm-cov and cargo-nextest)
test-coverage:
    cargo llvm-cov nextest --lcov --output-path lcov.info --ignore-filename-regex "src/main\.rs|src/bin/cli\.rs"

# Generate HTML coverage report
test-coverage-html:
    cargo llvm-cov nextest --html --ignore-filename-regex "src/main\.rs|src/bin/cli\.rs"

# Check coverage threshold (hard gate, 90%)
test-coverage-check:
    cargo llvm-cov --fail-under-lines 90 --ignore-filename-regex "src/main\.rs|src/bin/cli\.rs"

# Generate branch coverage report (HTML)
test-coverage-branch-html:
    cargo llvm-cov nextest --branch --html --ignore-filename-regex "src/main\.rs|src/bin/cli\.rs"

# Check branch coverage threshold (default 70%)
test-coverage-branch-check THRESHOLD="70":
    ./scripts/check-branch-coverage.sh {{THRESHOLD}}

# Full coverage report (line + branch)
test-coverage-full: test-coverage test-coverage-branch-html
    @echo "Full coverage report generated (line + branch)"

# Run tests matching pattern
test-package PATTERN:
    cargo nextest run -E '{{PATTERN}}'

# Run doc tests
test-doc:
    cargo test --doc

# Run specific test by name pattern
test-filter PATTERN:
    cargo nextest run -E '{{PATTERN}}'

# Run routing module tests
test-routing:
    cargo nextest run --lib -E 'test(routing)'

# Run registry module tests
test-registry:
    cargo nextest run --lib -E 'test(registry)'

# Run tracing module tests
test-tracing:
    cargo nextest run --lib -E 'test(tracing)'

# Run gateway core tests
test-gateway:
    cargo nextest run --lib -E 'not test(routing) and not test(registry) and not test(tracing) and not test(utils)'

# Review pending insta snapshots
test-snapshots:
    cargo insta review

# Accept all pending insta snapshots
test-snapshots-accept:
    cargo insta test --accept

# Run property-based tests
test-property:
    cargo nextest run -E 'test(proptests)'

# Run red/edge (failure-path) tests only
test-red:
    cargo nextest run -E 'test(red_edge) + test(proptests) + test(routes_input_validation) + test(db_resilience) + test(concurrency_error_paths)'

# Run happy-path tests only
test-happy:
    cargo nextest run -E 'not (test(red_edge) + test(proptests) + test(routes_input_validation) + test(db_resilience) + test(concurrency_error_paths))'

# Report red vs total test count
test-red-count:
    @bash scripts/count-red-tests.sh

# Run BDD scenarios (Gherkin feature files via cucumber)
bdd:
    cargo test --test cucumber_bdd --features bdd

# ============================================
# BENCHMARKS
# ============================================

# Run all benchmarks
bench:
    cargo bench

# Run specific benchmark
bench-target TARGET:
    cargo bench --bench {{TARGET}}

# Run benchmarks and save baseline
bench-save:
    cargo bench -- --save-baseline main

# ============================================
# ADVANCED TESTING TOOLS
# ============================================

# Install miri (requires nightly)
install-miri:
    rustup toolchain install nightly
    rustup component add miri --toolchain nightly

# Run miri on unsafe code tests
miri:
    MIRIFLAGS="-Zmiri-disable-isolation" cargo +nightly miri test --lib -E 'test(env)'

# Run miri on all lib tests (slow)
miri-full:
    MIRIFLAGS="-Zmiri-disable-isolation" cargo +nightly miri test --lib

# Install ThreadSanitizer (requires nightly + rust-src)
install-tsan:
    rustup toolchain install nightly
    rustup component add rust-src --toolchain nightly

# Run TSAN on concurrent tests (single-threaded to avoid false positives)
tsan:
    RUSTFLAGS="-Zsanitizer=thread" TSAN_OPTIONS="halt_on_error=1" cargo +nightly test --lib --target x86_64-unknown-linux-gnu -- --test-threads=1

# Run loom systematic concurrency tests
loom:
    cargo test --test loom_rate_limiter

# Run loom with verbose output to see interleavings
loom-verbose:
    RUST_LOG=loom cargo test --test loom_rate_limiter -- --nocapture

# Run shuttle randomized async concurrency tests (5000 iterations)
shuttle:
    cargo test --test shuttle_concurrency -- --shuttle-count 5000

# Run shuttle with more iterations for deeper testing
shuttle-deep:
    cargo test --test shuttle_concurrency -- --shuttle-count 10000

# Run gateway with tokio-console instrumentation
console:
    cargo run --features console --bin gateway

# Install tokio-console CLI
install-console:
    cargo install tokio-console

# Run error-path concurrency tests
test-concurrency-errors:
    cargo test --test concurrency_error_paths

# Install cargo-mutants
install-mutants:
    cargo install cargo-mutants

# Run mutation testing on concurrency-sensitive code
mutants:
    cargo mutants --in-place

# Run mutation testing on a specific file
mutants-file FILE:
    cargo mutants --file {{FILE}}

# Check mutation score without writing changes
mutants-check:
    cargo mutants --copy-target=false --check

# ============================================
# FUZZING (requires nightly)
# ============================================

# Install cargo-fuzz and nightly toolchain
install-fuzz:
    rustup toolchain install nightly
    cargo +nightly install cargo-fuzz

# Run SSRF URL fuzzer (60s default)
fuzz-ssrf:
    cargo +nightly fuzz run ssrf_url_fuzz -- -max_total_time=60

# Run config parse fuzzer (60s default)
fuzz-config:
    cargo +nightly fuzz run config_parse_fuzz -- -max_total_time=60

# Run token match fuzzer (60s default)
fuzz-token:
    cargo +nightly fuzz run token_match_fuzz -- -max_total_time=60

# Run all fuzzers sequentially (60s each)
fuzz-all: fuzz-ssrf fuzz-config fuzz-token
    @echo "All fuzz targets completed"

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
    cargo run --bin gateway-cli -- {{ARGS}}

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

# Show source structure
structure:
    @echo "📁 Source Structure:"
    @find src -name "*.rs" | head -40

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

# Pre-push: lint + BDD checks (matches lefthook)
pre-push: qa-bdd
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

# CI test (all features, nextest with CI profile)
ci-test:
    cargo nextest run --all-features --profile ci

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
    @ls -lh target/debug/{gateway,gateway-cli} 2>/dev/null || echo "Build first with: just build"

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
#   Tier 2.5 (BDD <20s): just qa-bdd      -> qa-lint, bdd
#   Tier 3 (Full >30s):  just qa-full      -> qa-lint, test, test-coverage-check, loom, shuttle, security-audit
#   Tier 4 (Security):   just qa-security  -> test, test-coverage-check, property tests, miri, tsan, audit
#   Tier 5 (Mutation):   just qa-mutation  -> qa-full, mutants-check
#
# Quick Development Flow:
#   just qa              # Quick feedback during development
#   just qa-lint         # Lint check before push
#   just test            # Run tests (nextest)
#   just qa-full         # Full verification before release
#   just qa-security     # Deep security verification
#   just qa-mutation     # Mutation testing for release candidates
#
# Coverage:
#   just test-coverage        # Generate lcov.info
#   just test-coverage-html   # Generate HTML report
#   just test-coverage-check  # Hard gate: fail if < 90%
#
# Benchmarks:
#   just bench                # Run all criterion benchmarks
#   just bench-save           # Run and save baseline
#
# Advanced Testing Tools:
#   just install-dev-all  # Install all dev tools (including advanced)
#   just miri             # Run miri on unsafe code tests
#   just tsan             # Run ThreadSanitizer on concurrent tests
#   just loom             # Run loom systematic concurrency tests
#   just shuttle          # Run shuttle randomized async concurrency tests
#   just mutants          # Run mutation testing
#   just console          # Run gateway with tokio-console
#
# Fuzzing (requires nightly):
#   just install-fuzz         # Install cargo-fuzz + nightly
#   just fuzz-all             # Run all fuzzers (60s each)
#   just fuzz-ssrf            # Run SSRF fuzzer only
#
# Snapshots:
#   just test-snapshots       # Review pending snapshots
#   just test-snapshots-accept # Accept all pending snapshots
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
