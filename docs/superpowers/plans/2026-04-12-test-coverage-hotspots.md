# Test Coverage Hotspots — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close coverage gaps on 8 files below 80% line coverage, keeping aggregate above 90%.

**Architecture:** Add focused unit tests to files with the lowest coverage. Work from easiest wins (policy templates, fallback config) to harder targets (SQLite store, selector). Each task targets one file and is independently committable.

**Tech Stack:** Rust test framework, rstest, proptest, pretty_assertions

---

## Coverage Gap Summary

| File | Current | Target | Priority |
|------|---------|--------|----------|
| `registry/policy/templates/mod.rs` | 48.39% | 90%+ | P2 (easy) |
| `routing/fallback/mod.rs` | 50.00% | 90%+ | P2 (easy) |
| `routing/sqlite/store/operations.rs` | 54.52% | 80%+ | P1 (critical) |
| `routing/selector/mod.rs` | 60.00% | 85%+ | P1 (critical) |
| `registry/policy/registry.rs` | 71.94% | 85%+ | P2 |
| `registry/policy/matcher.rs` | 72.61% | 85%+ | P2 |
| `routing/bandit/mod.rs` | 73.53% | 90%+ | P2 (easy) |
| `routing/history.rs` | 82.51% | 90%+ | P2 |

---

## Task 1: Policy Templates Tests (48% → 90%+)

**Files:**
- Modify: `src/registry/policy/templates/mod.rs` (add `#[cfg(test)]` module)

Each template function returns a `RoutingPolicy`. Test that each has correct name, priority, action, and conditions.

- [ ] Add test module with 9 tests (one per template function + prefer_provider variants)
- [ ] Run tests, verify pass
- [ ] Commit

## Task 2: Fallback Config Tests (50% → 90%+)

**Files:**
- Modify: `src/routing/fallback/mod.rs` (add `#[cfg(test)]` module — note: `tests` module already exists in `tests/` subdir)

Test `FallbackConfig::default()`, `FallbackPlanner::new()`, `with_config()`, `config()`, `set_config()`.

- [ ] Add inline test module for config/planner construction
- [ ] Run tests, verify pass
- [ ] Commit

## Task 3: Bandit Mod Tests (73% → 90%+)

**Files:**
- Modify: `src/routing/bandit/mod.rs` (add tests for constructors)

- [ ] Add tests for `BanditPolicy::new()`, record/get operations
- [ ] Run tests, verify pass
- [ ] Commit

## Task 4: SQLite Store Operations Tests (54% → 80%+)

**Files:**
- Modify: `src/routing/sqlite/tests.rs` (add new test module)

Add tests for: cache-enabled paths, `load_all_health`, error paths, concurrent writes, upsert behavior.

- [ ] Add tests for cache-enabled read/write paths
- [ ] Add tests for load_all_health and all_health edge cases
- [ ] Run tests, verify pass
- [ ] Commit

## Task 5: Selector Mod Tests (60% → 85%+)

**Files:**
- Modify: `src/routing/selector/tests.rs` (add new tests)

Add tests for: `with_policy()` constructor, selection with empty auths, selection with all unhealthy, policy-aware selection.

- [ ] Add tests for SmartSelector constructors and edge cases
- [ ] Run tests, verify pass
- [ ] Commit

## Task 6: Policy Matcher Tests (72% → 85%+)

**Files:**
- Modify: `src/registry/policy/matcher_tests.rs` (add new tests)

- [ ] Add tests for matcher edge cases and uncovered branches
- [ ] Run tests, verify pass
- [ ] Commit

## Task 7: Policy Registry Tests (71% → 85%+)

**Files:**
- Modify: `src/registry/policy/registry.rs` (add to existing `#[cfg(test)]` module)

- [ ] Add tests for uncovered registry operations
- [ ] Run tests, verify pass
- [ ] Commit

## Task 8: History Tests (82% → 90%+)

**Files:**
- Modify: `src/routing/history.rs` (add to existing `#[cfg(test)]` module)

- [ ] Add tests for uncovered history branches
- [ ] Run tests, verify pass
- [ ] Commit

---

## Verification

After all tasks:

```bash
cargo nextest run
cargo llvm-cov --fail-under-lines 90 --ignore-filename-regex "src/main\.rs|src/bin/cli\.rs"
cargo clippy --all-targets -- -D warnings
```
