---
name: repo-config-reviewer
description: Review repository configuration files (e.g. .pre-commit-config.yaml, .github/workflows/*.yml) against defined standards. Use this skill when asked to check, audit, or review repository settings, hooks, or CI/CD pipelines.
---

# Repo Config Reviewer

## Overview

This skill enables Claude to audit repository configuration files, ensuring adherence to "First Principles" of Agentic Coding and operational best practices. It helps identify inefficiencies, security risks, and deviations from standardized hook and CI strategies.

## Core Capabilities

### 1. Hook System Analysis
- **Audit `pre-commit`**: Verify it only runs fast checks (linting, formatting, unit tests).
- **Audit `pre-push`**: Verify it includes comprehensive checks (strict types, security, full test suite).
- **Tool Check**: Ensure standard tools (`ruff`, `black`, `isort`, `mypy`) use correct arguments (e.g., `--strict` for mypy).

### 2. CI/CD Workflow Review
- **SSoT Enforcement**: Flag ANY direct tool use (e.g., `npm run test` or `poetry run pytest`) in CI. Require `just test`.
- **Consistency**: Ensure CI workflows match local `just` commands (Single Source of Truth).
- **Coverage**: Verify CI pipelines run the full qa suite (`qa-full`).

### 3. Standards Compliance
- **Safety**: Ensure `gitleaks` and `bandit` are present and correctly configured.
- **Performance**: Flag slow checks in the wrong hooks (e.g., mypy in pre-commit).
- **Agent Alignment**: Ensure configurations are "Agent Friendly" (e.g., using `just` as a unified interface).

## Usage

When asked to review repository settings:

1.  **Identify Targets**: Locate relevant config files (`.pre-commit-config.yaml`, `.github/workflows/*`, `justfile`).
2.  **Compare against Standards**: Use `references/standards.md` as the baseline.
3.  **Report Findings**:
    *   **Pass**: Configuration meets standards.
    *   **Fail/Warning**: Deviation found (explain *why* it matters).
    *   **Recommendation**: Specific fix or alignment.

## Resource Organization

### references/
- `standards.md`: The "Gold Standard" configuration patterns for hooks and CI/CD.
