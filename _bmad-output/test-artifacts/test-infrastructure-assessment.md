# Test Infrastructure Assessment

**Assessed by:** Murat (TEA — Master Test Architect)
**Date:** 2026-04-12
**Project:** router-gateway (local LLM gateway, Rust)

---

## Strengths (Low Risk)

| Area | Assessment |
|------|-----------|
| **Test Pyramid Shape** | Solid. 50+ unit test modules, 6 integration test files, BDD feature files. Weighted toward lower levels. |
| **Coverage Gate** | 80% threshold enforced in CI via tarpaulin + Codecov. Right number for a Rust backend service. |
| **CI Pipeline Design** | Tiered gates well-architected — fast quality (<2 min) runs before tests. MSRV check is valuable. |
| **Multi-OS Matrix** | ubuntu/macos/windows. Critical for a tool people run locally. |
| **Lint Discipline** | clippy strict + deny, workspace lint inheritance enforcement, clippy-allow audit in CI. |
| **Dev Dependencies** | Well-chosen: `rstest` for parameterized tests, `pretty_assertions` for diffs, `wiremock` for HTTP mocking, `assert_cmd` for CLI testing. |
| **Security Pipeline** | Gitleaks, cargo-audit, cargo-deny, CodeQL. Defense in depth. |
| **Justfile Ergonomics** | `just qa` vs `just qa-full` gives developers fast feedback loops. Tiered verification is the right pattern. |

---

## Risk Assessment

| # | Risk | Category | P | I | Score | Detail |
|---|------|----------|---|---|-------|--------|
| 1 | **No property-based testing** | TECH | 2 | 2 | **4** | `rstest` covers parameterized tests but no `proptest` or `quickcheck`. For routing algorithms with float-based metrics (bandit, weights, health scores), property-based testing catches NaN/ordering edge cases the known-pitfall doc warns about. |
| 2 | **BDD tests are monolithic** | TECH | 2 | 2 | **4** | `tests/bdd_integration_tests.rs` is 1,275 lines. `crates/gateway/tests/routes.rs` is 1,577 lines. Test quality standard caps at 300 lines per file. Maintenance and merge conflict risk. |
| 3 | **No test fixture sharing across crates** | TECH | 2 | 2 | **4** | Each crate defines test helpers independently. No shared test utility crate. Duplication grows with workspace size. |
| 4 | **Coverage excludes main.rs but not other bins** | OPS | 1 | 2 | **2** | tarpaulin excludes `cli/src/main.rs` and `gateway/src/main.rs`. Verify no other entry points need exclusion/inclusion. |
| 5 | **No mutation testing** | TECH | 2 | 1 | **2** | 80% coverage doesn't mean 80% effective. `cargo-mutants` would validate tests catch bugs, not just execute lines. Nice-to-have. |
| 6 | **No snapshot testing** | TECH | 1 | 2 | **2** | `cargo insta` mentioned in testing instructions but not in dev-dependencies. High-value for API response and config parsing validation. |

### Critical Gaps (Score >= 6)

None identified. Test infrastructure is mature for the project's stage.

---

## Priority-Ordered Recommendations

1. **Add `proptest` to `smart-routing`** — Bandit/weight/health scoring uses float comparisons. The codebase already documents NaN pitfalls. Property-based testing gives exponential coverage gains for numeric edge cases. Effort: 1-2 sessions.

2. **Split monolithic test files** — Break `bdd_integration_tests.rs` into domain modules (classification, health, planning, execution, learning). Break `routes.rs` into endpoint-group modules. Maintains scenario value, reduces cognitive load and merge conflict risk.

3. **Consider shared `test-utils` crate** — When 3+ crates duplicate fixture patterns, a workspace test utility crate pays for itself. Plan now, implement when duplication becomes painful.

4. **Add `insta` for snapshot testing** — Particularly valuable for `gateway/tests/config.rs` (492 lines). Snapshot tests would cut file size while improving assertion coverage.

---

## Actions Taken

Based on this assessment, the following infrastructure changes were made:

- Simplified clippy configuration to `all` + `unwrap_used` + `expect_used` (removed 30+ unexplained overrides)
- Replaced pre-commit framework with lefthook for git hooks
- Established three-layer verification: pre-commit (fmt + check -q) / pre-push (clippy) / CI (full)
- Updated justfile tiered verification to match three-layer model
