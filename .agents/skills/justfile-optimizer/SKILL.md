---
name: justfile-optimizer
description: Optimize `justfile` configurations by implementing tiered command structures (Fast vs Slow) and establishing `just` as the Unified Command Interface (SSoT).
---

# Justfile Optimizer

## Overview

This skill helps refactor project automation into a **Tiered Verification System** using `just`. It transforms monolithic or scattered scripts (like `npm run` sprawl) into a structured, unified interface.

## Core Principles

### 1. Tiered Verification
- **Strategy**: Split checks based on execution cost.
- **Tier 1 (Fast)**: `< 5s`. Linting, formatting, basic unit tests. Blocks commits.
- **Tier 2 (Slow)**: `> 5s`. Types, Security, Integration Tests, Builds. Blocks push/deploy.

### 2. Single Source of Truth (SSoT)
- **Rule**: `just` is the **only** entry point for developer and CI commands.
- **Pattern**: `CI Workflow` -> calls -> `just qa-full`.

## Usage

When asked to optimize a repository's automation:

1.  **Audit**: List current scripts in `package.json`, `Makefile`, etc.
2.  **Categorize**: Label each check as **Fast** or **Slow**.
3.  **Refactor**: Create/Update `justfile` with tiered recipes.

### Tiered Recipe Pattern

```just
# Tier 1: Fast Feedback (Use for pre-commit)
qa:
    just fmt-check
    just lint
    just test-fast

# Tier 2: Comprehensive (Use for pre-push / CI)
qa-full:
    just qa
    just type-check
    just security-scan
    just test-full
```

## Resources

### references/
- `justfile-syntax.md`: Syntax guide for recipes, variables, dependencies, and cross-platform compatibility.
