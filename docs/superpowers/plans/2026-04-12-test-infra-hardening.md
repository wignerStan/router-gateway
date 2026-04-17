# Test Infrastructure Hardening — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove `just` from CI (direct cargo), raise coverage gate to 90%, implement deferred test hardening (rstest, insta, proptest), and update documentation.

**Architecture:** Four independent workstreams executed sequentially for clean commits: (1) CI decoupling from just, (2) coverage threshold increase, (3) test hardening with modern Rust test libraries, (4) documentation updates.

**Tech Stack:** cargo-nextest 0.9.92, cargo-llvm-cov 0.8.5, rstest 0.25, insta 1.42, proptest 1.6

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `.github/workflows/ci.yml` | MODIFY | Remove all `just` install/usage, direct cargo commands |
| `codecov.yml` | MODIFY | Raise thresholds to 90% |
| `lefthook.yml` | MODIFY | Add local coverage check (optional, lower threshold) |
| `tests/config.rs` | MODIFY | Add insta snapshots, rstest parameterization |
| `src/routing/bandit/tests.rs` | MODIFY | Add proptest module |
| `src/routing/weight/tests.rs` | MODIFY | Add proptest module |
| `src/routing/health/tests.rs` | MODIFY | Add rstest parameterization, proptest module |
| `AGENTS.md` | MODIFY | Update build/test table, add nextest/coverage docs |

---

## Workstream 1: Remove `just` from CI

### Task 1.1: Replace just invocations with direct cargo

**Files:**
- Modify: `.github/workflows/ci.yml`

Current CI uses `just` in 4 jobs. Replace each with the equivalent cargo command.

- [ ] **Step 1: Replace `lint` job — remove just, use direct clippy**

In `.github/workflows/ci.yml`, in the `lint` job, remove the "Install just" step and change the run command:

```yaml
      # REMOVE this entire step:
      # - name: Install just
      #   uses: taiki-e/install-action@v2
      #   with:
      #     tool: just

      - name: Run clippy (strict)
        run: cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 2: Replace `type-check` job — remove just, use direct cargo check**

In the `type-check` job, remove the "Install just" step and change the run command:

```yaml
      - name: Run type check
        run: cargo check --all --all-features
```

- [ ] **Step 3: Remove unused just install from `build` job**

In the `build` job, remove the "Install just" step entirely. The build job only uses `cargo build --release`.

- [ ] **Step 4: Replace `docs` job — remove just, use direct cargo doc**

In the `docs` job, remove the "Install just" step and change the run command:

```yaml
      - name: Build documentation
        run: cargo doc --no-deps --all
```

- [ ] **Step 5: Verify CI YAML is valid**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"`
Expected: no output (valid YAML)

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "refactor: remove just dependency from CI, use direct cargo commands"
```

---

## Workstream 2: Raise Coverage to 90%

### Task 2.1: Update codecov.yml to 90%

**Files:**
- Modify: `codecov.yml`

- [ ] **Step 1: Raise thresholds**

Replace `codecov.yml` entirely:

```yaml
coverage:
  status:
    project:
      default:
        target: 90%
        threshold: 1%
        if_ci_failed: error
    patch:
      default:
        target: 90%
        threshold: 5%

comment:
  layout: "header, diff, files"
  behavior: once

ignore:
  - "src/main.rs"
  - "src/bin/cli.rs"
  - "tests/**"
```

- [ ] **Step 2: Update CI coverage threshold gate**

In `.github/workflows/ci.yml`, in the `coverage` job, change `--fail-under-lines 80` to `--fail-under-lines 90`:

```yaml
      - name: Coverage threshold gate (90%)
        run: |
          cargo llvm-cov \
            --fail-under-lines 90 \
            --ignore-filename-regex "src/main\.rs|src/bin/cli\.rs"
```

- [ ] **Step 3: Update justfile local coverage check**

In `justfile`, change the `test-coverage-check` recipe from 80 to 90:

```just
# Check coverage threshold (hard gate, 90%)
test-coverage-check:
    cargo llvm-cov --fail-under-lines 90 --ignore-filename-regex "src/main\.rs|src/bin/cli\.rs"
```

- [ ] **Step 4: Update lefthook.yml for optional local lower threshold**

Add a commented-out coverage hook in `lefthook.yml` after the pre-push section:

```yaml
pre-push:
  commands:
    clippy:
      run: cargo clippy --all-targets -- -D warnings
    # Optional: uncomment for local coverage gate (lower threshold for speed)
    # coverage:
    #   run: cargo llvm-cov --fail-under-lines 80 --ignore-filename-regex "src/main\.rs|src/bin/cli\.rs"
```

- [ ] **Step 5: Verify coverage meets 90%**

Run: `cargo llvm-cov --fail-under-lines 90 --ignore-filename-regex "src/main\.rs|src/bin/cli\.rs" > /dev/null 2>&1; echo "exit: $?"`
Expected: `exit: 0` (current coverage is ~92%)

- [ ] **Step 6: Commit**

```bash
git add codecov.yml .github/workflows/ci.yml justfile lefthook.yml
git commit -m "feat: raise coverage gate from 80% to 90%"
```

---

## Workstream 3: Test Hardening

### Task 3.1: Add rstest parameterization to config validation tests

**Files:**
- Modify: `tests/config.rs:1-10` (add imports)
- Modify: `tests/config.rs:46-81` (consolidate 3 error tests)
- Modify: `tests/config.rs:420-444` (consolidate 2 validation tests)

These tests all follow the same pattern: parse YAML → assert error → check message substring. Consolidate with `#[rstest]`.

- [ ] **Step 1: Add rstest import to tests/config.rs**

At the top of `tests/config.rs`, add after existing imports:

```rust
use rstest::rstest;
```

- [ ] **Step 2: Write the failing parameterized config error test**

Add a new test at line ~46 (before `test_duplicate_credential_id_fails`):

```rust
#[rstest]
#[case::duplicate_id(
    indoc::indoc! {"
        credentials:
          - id: cred1
            api_key: key1
            provider: openai
          - id: cred1
            api_key: key2
            provider: openai
    "},
    "duplicate credential id"
)]
#[case::invalid_strategy(
    indoc::indoc! {"
        routing:
          strategy: invalid_strategy
        credentials:
          - id: cred1
            api_key: key1
            provider: openai
    "},
    "invalid routing strategy"
)]
#[case::empty_api_key(
    indoc::indoc! {"
        credentials:
          - id: cred1
            api_key: ''
            provider: openai
    "},
    "api_key must not be empty"
)]
#[case::empty_provider(
    indoc::indoc! {"
        credentials:
          - id: cred1
            api_key: key1
            provider: ''
    "},
    "provider must not be empty"
)]
fn test_config_validation_errors(#[case] yaml_content: &str, #[case] expected_message: &str) {
    let result = config_from_yaml_content(yaml_content);
    assert!(result.is_err(), "Expected error containing '{}', got {:?}", expected_message, result);
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.to_lowercase().contains(expected_message),
        "Error '{}' did not contain expected message '{}'",
        err_msg,
        expected_message
    );
}
```

Note: Check if `indoc` is available. If not, use raw strings.

- [ ] **Step 3: Run test to verify it compiles and passes**

Run: `cargo nextest run -E 'test(test_config_validation_errors)'`
Expected: PASS (4 test cases)

- [ ] **Step 4: Remove the old individual tests**

Remove the following functions from `tests/config.rs`:
- `test_duplicate_credential_id_fails` (lines 46-65)
- `test_invalid_strategy_fails` (lines 67-81)
- `test_empty_api_key_fails_validation` (lines 420-431)
- `test_empty_provider_fails_validation` (lines 433-444)

- [ ] **Step 5: Run full config tests**

Run: `cargo nextest run -E 'test(config)'`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add tests/config.rs
git commit -m "refactor: parameterize config validation tests with rstest"
```

### Task 3.2: Add insta snapshots to config parsing tests

**Files:**
- Modify: `tests/config.rs` (add import, convert 3 tests)
- Create: `tests/snapshots/` (auto-generated by insta)

Target tests with extensive field-by-field assertions:
- `test_full_config_with_all_sections` (lines 248-330, 15+ assertions)
- `test_credential_with_all_fields` (lines 339-370, 9 assertions)
- `test_multiple_providers_config` (lines 372-418, 8 assertions)

- [ ] **Step 1: Add insta import**

At the top of `tests/config.rs`:

```rust
use insta::assert_yaml_snapshot;
```

- [ ] **Step 2: Write the snapshot tests**

Replace `test_full_config_with_all_sections` body (after parsing the config) with:

```rust
#[test]
fn test_full_config_with_all_sections() {
    let yaml_content = r#"
    # ... keep existing YAML content ...
    "#;
    let config = config_from_yaml_content(yaml_content);
    assert_yaml_snapshot!("full_config_all_sections", &config);
}
```

Repeat for `test_credential_with_all_fields` → snapshot name `"credential_all_fields"`.
Repeat for `test_multiple_providers_config` → snapshot name `"multiple_providers_config"`.

- [ ] **Step 3: Generate and accept initial snapshots**

Run: `cargo insta test --accept`
This creates snapshot files in `tests/snapshots/`.

- [ ] **Step 4: Verify all config tests pass**

Run: `cargo nextest run -E 'test(config)'`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add tests/config.rs tests/snapshots/
git commit -m "feat: add insta snapshot tests for config parsing"
```

### Task 3.3: Add proptest to bandit sampling

**Files:**
- Modify: `src/routing/bandit/tests.rs` (add proptest module at end)

The bandit module has `sample_beta(alpha, beta) -> f64` and `sample_gamma(shape) -> f64` that must produce values in valid ranges for any positive inputs. Existing tests check specific values; proptest will check all values.

- [ ] **Step 1: Add proptest module to bandit tests**

At the end of `src/routing/bandit/tests.rs`, add:

```rust
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn beta_sample_always_in_unit_interval(
            alpha in 0.001_f64..1000.0,
            beta in 0.001_f64..1000.0,
        ) {
            let sample = BanditPolicy::sample_beta(alpha, beta);
            prop_assert!(sample >= 0.0 && sample <= 1.0,
                "Beta({}, {}) = {}, expected [0, 1]", alpha, beta, sample);
        }

        #[test]
        fn gamma_sample_always_positive_and_finite(
            shape in 0.001_f64..100.0,
        ) {
            let sample = BanditPolicy::sample_gamma(shape);
            prop_assert!(sample > 0.0 && sample.is_finite(),
                "Gamma({}) = {}, expected positive finite", shape, sample);
        }

        #[test]
        fn record_result_never_panics(
            utility in prop_oneof![
                Just(f64::NAN),
                Just(f64::INFINITY),
                Just(f64::NEG_INFINITY),
                Just(0.0_f64),
                Just(-0.0_f64),
                any::<f64>(),
            ],
            success: bool,
        ) {
            let mut policy = BanditPolicy::new();
            policy.record_result("route", success, utility);
            // If we get here, no panic occurred
        }
    }
}
```

Note: `BanditPolicy::sample_beta` and `BanditPolicy::sample_gamma` must be accessible from the test module. Verify they are `pub(crate)` or `pub`. If not, the existing tests already call them, so they must be accessible via `use super::*`.

- [ ] **Step 2: Run proptests**

Run: `cargo nextest run -E 'test(bandit::proptests)'`
Expected: PASS (64 cases per test)

- [ ] **Step 3: Run all bandit tests to confirm no regressions**

Run: `cargo nextest run -E 'test(bandit)'`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add src/routing/bandit/tests.rs
git commit -m "feat: add proptest property tests for bandit sampling"
```

### Task 3.4: Add proptest to weight calculation

**Files:**
- Modify: `src/routing/weight/tests.rs` (add proptest module at end)

The weight calculator must produce non-negative, non-NaN values for any input combination.

- [ ] **Step 1: Add proptest module to weight tests**

At the end of `src/routing/weight/tests.rs`, add:

```rust
mod proptests {
    use super::*;
    use proptest::prelude::*;

    fn arb_auth_info() -> impl Strategy<Value = AuthInfo> {
        (any::<Option<i32>>(), any::<bool>(), any::<bool>())
            .prop_map(|(priority, quota_exceeded, unavailable)| AuthInfo {
                id: "test-auth".to_string(),
                priority,
                quota_exceeded,
                unavailable,
                model_states: Vec::new(),
            })
    }

    fn arb_metrics() -> impl Strategy<Value = AuthMetrics> {
        (
            0u64..10000,
            0.0_f64..1.0,
            prop_oneof![Just(f64::NAN), Just(f64::INFINITY), 0.0_f64..100000.0],
        ).prop_map(|(total, success_rate, latency)| {
            let success_count = (total as f64 * success_rate) as u64;
            AuthMetrics {
                total_requests: total,
                success_count,
                failure_count: total.saturating_sub(success_count),
                avg_latency_ms: latency,
                min_latency_ms: latency * 0.5,
                max_latency_ms: latency * 2.0,
                success_rate,
                error_rate: 1.0 - success_rate,
                consecutive_successes: 0,
                consecutive_failures: 0,
                last_request_time: chrono::Utc::now(),
                last_success_time: Some(chrono::Utc::now()),
                last_failure_time: None,
            }
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn weight_always_non_negative(
            auth in arb_auth_info(),
            metrics in arb_metrics(),
        ) {
            let config = WeightConfig::default();
            let calculator = DefaultWeightCalculator::new(config);
            for status in [HealthStatus::Healthy, HealthStatus::Degraded, HealthStatus::Unhealthy] {
                let weight = calculator.calculate(&auth, Some(&metrics), status);
                prop_assert!(weight >= 0.0,
                    "Weight {} is negative for status {:?}", weight, status);
                prop_assert!(!weight.is_nan(),
                    "Weight is NaN for status {:?}", status);
            }
        }

        #[test]
        fn weight_bounded_above(
            auth in arb_auth_info(),
            metrics in arb_metrics(),
        ) {
            let config = WeightConfig::default();
            let calculator = DefaultWeightCalculator::new(config);
            let weight = calculator.calculate(&auth, Some(&metrics), HealthStatus::Healthy);
            prop_assert!(weight <= 2.0,
                "Weight {} exceeds reasonable bound for Healthy status", weight);
        }
    }
}
```

Note: Check `AuthMetrics` construction. The struct has `chrono::DateTime<Utc>` fields that may need specific construction. If the struct doesn't derive `Debug` or if field names differ, adapt accordingly. Look at existing tests (e.g., lines 14-46) for the exact construction pattern.

- [ ] **Step 2: Run proptests**

Run: `cargo nextest run -E 'test(weight::proptests)'`
Expected: PASS (64 cases per test)

- [ ] **Step 3: Run all weight tests**

Run: `cargo nextest run -E 'test(weight)'`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add src/routing/weight/tests.rs
git commit -m "feat: add proptest property tests for weight calculation"
```

### Task 3.5: Add proptest to health manager

**Files:**
- Modify: `src/routing/health/tests.rs` (add proptest module at end)

The health manager must reset consecutive failure counts on success, regardless of threshold or failure count.

- [ ] **Step 1: Add proptest module to health tests**

At the end of `src/routing/health/tests.rs`, add:

```rust
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[tokio::test]
        async fn consecutive_failures_reset_on_success(
            threshold in 1u32..100,
            failures in 1u32..200,
        ) {
            let config = HealthConfig {
                unhealthy_threshold: threshold,
                ..Default::default()
            };
            let manager = HealthManager::new(config);

            for _ in 0..failures {
                manager.update_from_result("auth", false, 500).await;
            }
            manager.update_from_result("auth", true, 200).await;

            let health = manager.get_health("auth").await.unwrap();
            prop_assert_eq!(health.consecutive_failures, 0);
            prop_assert_eq!(health.consecutive_successes, 1);
        }
    }
}
```

Note: `HealthManager` methods are async. Check existing tests for the exact pattern — they use `#[tokio::test]`. The `get_health` return type may be `Option<Health>` with a struct that has `consecutive_failures` and `consecutive_successes` fields. Check existing tests (e.g., lines 9-21) for the exact field access pattern. Adapt field names if needed.

- [ ] **Step 2: Run proptests**

Run: `cargo nextest run -E 'test(health::proptests)'`
Expected: PASS (32 cases)

- [ ] **Step 3: Run all health tests**

Run: `cargo nextest run -E 'test(health)'`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add src/routing/health/tests.rs
git commit -m "feat: add proptest property tests for health manager"
```

---

## Workstream 4: Documentation

### Task 4.1: Update AGENTS.md

**Files:**
- Modify: `AGENTS.md`

- [ ] **Step 1: Update the Build and Test Commands table (lines 14-23)**

Replace the table:

```markdown
| Task              | Command                                      |
| ----------------- | -------------------------------------------- |
| Build             | `cargo build`                                |
| Test              | `cargo nextest run`                          |
| Test (unit only)  | `cargo nextest run --lib`                    |
| Run Gateway       | `cargo run --bin gateway`                    |
| Lint              | `cargo clippy --all-targets -- -D warnings`  |
| Format            | `cargo fmt --all`                            |
| Coverage          | `cargo llvm-cov nextest --html`              |
| Coverage gate     | `cargo llvm-cov --fail-under-lines 90`       |
| Quick Checks      | `just qa`                                    |
| Full Checks       | `just qa-full`                               |
```

- [ ] **Step 2: Update Testing Instructions section**

Add after the existing testing bullet points:

```markdown
- **Test Runner**: Use `cargo nextest run` (not `cargo test`). Nextest provides faster parallel execution and better output. CI uses the `ci` profile with retries.
- **Coverage**: Minimum 90% line coverage enforced in CI via `cargo llvm-cov`. Coverage excludes `src/main.rs` and `src/bin/cli.rs`.
- **Snapshot Testing**: Use `insta` for structured output validation. Run `cargo insta test` then `cargo insta review` to review snapshots.
- **Property-Based Testing**: Use `proptest` for numeric edge cases. All float-heavy modules (bandit, weight) have proptest suites.
```

- [ ] **Step 3: Commit**

```bash
git add AGENTS.md
git commit -m "docs: update AGENTS.md with nextest, coverage, and test tooling"
```

---

## Verification

After all tasks are complete, run:

```bash
cargo nextest run                                    # All tests pass
cargo llvm-cov --fail-under-lines 90 --ignore-filename-regex "src/main\.rs|src/bin/cli\.rs"  # Coverage gate
cargo clippy --all-targets -- -D warnings            # No new lint warnings
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"  # Valid CI YAML
```
