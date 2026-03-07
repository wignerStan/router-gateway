---
name: repo-structure-advisor
description: Analyzes and advises on repository structure according to "Agentic Native" principles (Monorepo, Feature-First, Triad Docs). Use this when the user asks about repo organization, best practices for folder structure, or wants to validate their current layout.
---

# Repo Structure Advisor

This skill helps validate and advise on repository structure to ensure it is "Agentic Native"—meaning it is optimized for AI agents to understand, navigate, and modify safely.

## Core Principles

1.  **Unified Monorepo**: Separate `apps/` (deployable) and `packages/` (shared libraries).
2.  **Feature-First**: Organize code by feature (e.g., `features/auth/`), not file type (`controllers/`).
3.  **Triad Documentation**: Every module needs valid Code + Local README (Context) + Tests (Usage).

## Usage

### 1. Analyzer Script
Run the analysis script to get a quick health check of the current repository structure:

```bash
python3 .agent/skills/repo-structure-advisor/scripts/analyze_structure.py
```

### 2. Reference Standards
Read the detailed principles document to understand the "Why" and "How":

- [Agentic Structure Principles](references/agentic_structure.md)

## Common Advice Patterns

- **If `apps/` is missing**: Suggest moving top-level application code into `apps/<app-name>`.
- **If `packages/` is missing**: Suggest extracting shared logic (utils, UI, types) into `packages/<pkg-name>`.
- **If "File Type" folders exist (controllers/models)**: Strongly advise refactoring to "Feature-First" (`features/user/`).
- **If `justfile` is missing**: Advise creating a Unified Command Interface.
- **If tests are scattered or missing**: Suggest organizing by scope: Co-located Unit Tests (Triad), BDD Logic Tests, and Root-level E2E Tests.
- **If code is heavily commented**: Check if names/types can replace comments. If not, verify if comments follow BDD/Architecture pointer styles.

