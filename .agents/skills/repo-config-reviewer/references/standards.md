# Repository Configuration Standards

This document defines the "Gold Standard" for repository configuration, aligned with the First Principles of Agentic Coding.

## 1. Git Hooks Strategy (`.pre-commit-config.yaml`)

### Philosophy: Tiered Verification
- **Pre-commit**: Must be fast (<5s). Failure blocks commit.
- **Pre-push**: Can be slow. Failure blocks push.

### Standard Configuration

| Stage | Checks Allowed | Checks Forbidden | Rationale |
| :--- | :--- | :--- | :--- |
| **pre-commit** | `ruff`, `black`, `isort`, `prettier`, `bun-lint`, Fast Unit Tests (pytest/bun test) | `mypy`, `bandit`, `safety`, `gitleaks`, Full Test Suite | Developers need instant feedback loop. Slow checks break flow. |
| **pre-push** | `mypy` (strict), `bandit`, `safety`, `gitleaks`, `pytest-coverage` (Full), `bun-build`, `bun-type-check` | N/A | High-latency checks belong here to ensure "Main branch is always safe". |

### Agent-Friendly Note
Tools should be configured to output actionable details (e.g., `pass_filenames: false` for full scans in pre-push).

## 2. CI/CD Workflow (`.github/workflows/*.yml`)

### Philosophy: Single Source of Truth
- CI workflows must **mirror** local development commands.
- Do not duplicate logic between `justfile` and `ci.yml`.

### Standard Configuration

#### Correct Pattern (Using `just`)
```yaml
steps:
  - name: Install just
    uses: extractions/setup-just@v1
  - name: Run Quality Checks
    run: just qa-full
```

#### Incorrect Pattern (Duplicate Logic)
```yaml
steps:
  - run: poetry run ruff check .
  - run: poetry run mypy .
  # Bad: if local `just` flags change, CI drifts apart.
```

### Required Jobs
- **Test Matrix**: Run across multiple OS/Versions (e.g., Python 3.9-3.14, Node 18-20).
- **Security**: Run `gitleaks` and `bandit` (if Python).

## 3. Automation Interface (`justfile`)

### Philosophy: Single Source of Truth (SSoT)
- **Unified Endpoint**: All developer and CI commands MUST go through `just`.
- **Prohibited**: Direct calls to `npm`, `poetry`, `make`, or script files in CI or Hooks (unless `just` is unavailable, which should be rare).

### Rationale
- **Consistency**: Discrepancies between "how I run it" and "how CI runs it" are a major source of bugs.
- **Context Simplicity**: The Agent only needs to learn one syntax (`just <command>`), not the nuances of `npm run`, `yarn`, `pnpm`, `poetry`, `cargo`, etc.

### Standard Commands
- `just install`: Install dependencies.
- `just dev`: Start dev server.
- `just test`: Run standard tests.
- `just fmt`: Format code.
- `just lint`: Lint code.
- `just qa`: Run fast verification (pre-commit equivalent).
- `just qa-full`: Run full verification (pre-push/CI equivalent).
