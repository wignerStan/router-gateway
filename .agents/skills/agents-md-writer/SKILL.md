---
name: agents-md-writer
description: Create and update AGENTS.md (or Rules) files for AI coding assistants. Apply first principles of Agentic Coding to write effective, concise project configuration that maximizes agent performance.
---

# AGENTS.md Writer

## Overview

This skill guides the creation and maintenance of `AGENTS.md` (or equivalent configuration files like `Rules`, `.cursorrules`, `CLAUDE.md`) for AI coding assistants. These files act as the **onboarding document for every AI session**, injected into every conversation's context.

## Core Principles

### 1. The Stateless Reality

LLMs are **stateless functions**. The agent's entire understanding of your codebase comes from the tokens you put in the context window. Every new session starts from zero.

**Implications:**
- Treat `AGENTS.md` as the **"onboarding document for every session"**
- Any critical project knowledge must be documented here or referenced
- Information not in the context window **does not exist** for the agent

### 2. Less is More

Research shows frontier models reliably follow ~150-200 instructions. The agent's system prompt already consumes ~50 of these. **Every additional line competes for attention.**

**Targets:**
- **Ideal**: < 100 lines
- **Acceptable**: < 300 lines
- **Danger Zone**: > 300 lines (diminishing returns)

**Anti-Pattern:**
> ❌ Writing a comprehensive style guide with 50 formatting rules
> 
> ✅ Configure linters/formatters to auto-fix; LLMs are context learners

### 3. Progressive Disclosure

Don't dump everything into `AGENTS.md`. Use a **pointer-based architecture**:

```
AGENTS.md                 # Core instructions (~100 lines)
└── References:
    ├── agent_docs/building.md
    ├── agent_docs/testing.md
    ├── agent_docs/architecture.md
    └── agent_docs/conventions.md
```

Let the agent **pull relevant docs on demand** based on the current task.

---

## Content Structure

### Required Sections

#### 1. **WHAT** — Project Identity

```markdown
## Project Overview
- **Type**: [e.g., Web API, CLI tool, Monorepo]
- **Stack**: [e.g., Python 3.12, FastAPI, PostgreSQL]
- **Structure**:
  - `src/` - Application code
  - `tests/` - Test suite
  - `scripts/` - Automation utilities
```

#### 2. **WHY** — Design Context

```markdown
## Design Decisions
- Using `pydantic` for all data validation (not dataclasses)
- Database access via `sqlalchemy` async sessions only
- See `docs/ADR/001-why-pydantic.md` for rationale
```

#### 3. **HOW** — Operational Commands

```markdown
## Quick Start
- **Install**: `pnpm install` (NOT npm or yarn)
- **Test**: `just test` (runs fast tests < 5s)
- **Full QA**: `just qa-full` (runs all checks)
- **Format**: `just fmt` (auto-fix style issues)
```

### Optional Sections

#### 4. **Known Pitfalls** (High Value)

```markdown
## ⚠️ Common Mistakes
- `session.save()` is async — always `await` it
- Never import from `internal/` outside the module
- API rate limits: max 100 req/min per client
```

#### 5. **Successful Patterns** (Reference for Quality)

```markdown
## ✅ Reference Implementations
- New API endpoint: See `src/api/users.py`
- Error handling: Follow pattern in PR #241
- Testing strategy: Reference `tests/integration/test_auth.py`
```

#### 6. **Agent-Specific Guidance**

```markdown
## 🤖 Agent Guidelines
- Prefer modifying existing files over creating new ones
- Run `just test` after every code change
- Commit frequently with descriptive messages
- Ask before deleting any files
```

---

## What NOT to Include

### 1. Linting Rules
> **Bad**: "Use 4-space indentation, no trailing whitespace..."
> 
> **Good**: Configure `ruff`, `eslint`, `prettier` — let tools handle formatting

### 2. Duplicate Documentation
> **Bad**: Copying code samples that will become stale
> 
> **Good**: `See src/api/users.py:45-60 for the pattern`

### 3. Rarely-Used Information
> **Bad**: Database migration procedures (used once a month)
> 
> **Good**: Reference in `agent_docs/migrations.md`, list in index

### 4. General Programming Advice
> **Bad**: "Write clean code, use meaningful names..."
> 
> **Good**: Project-specific conventions only

---

## Writing Style

### Be Declarative, Not Descriptive

```markdown
# ❌ Descriptive (wastes tokens)
"We have decided to use TypeScript for this project because 
it provides type safety and better IDE support..."

# ✅ Declarative (actionable)
**Stack**: TypeScript 5.0+ with strict mode enabled.
```

### Prefer Commands Over Explanations

```markdown
# ❌ Explaining
"To run the tests, you should navigate to the project root
and use the just command..."

# ✅ Command
**Test**: `just test`
```

### Use Structured Formats

```markdown
# ❌ Prose
The project uses pnpm for package management, jest for testing,
and prettier for formatting.

# ✅ Structured
| Task | Command |
|------|---------|
| Install | `pnpm install` |
| Test | `pnpm test` |
| Format | `pnpm fmt` |
```

---

## Maintenance Strategy

### 1. Compounding Engineering

Turn every learning into permanent knowledge:

- **Bug Fixed?** → Add to "Known Pitfalls" if it could recur
- **Code Review Comment?** → Generalize and add to conventions
- **Agent Confusion?** → Clarify in AGENTS.md or add pointer

### 2. Regular Pruning

- Remove outdated information quarterly
- Check file:line references still valid
- Merge redundant sections

### 3. Version Control

Track changes to `AGENTS.md` with meaningful commits:
```
git log --oneline -- AGENTS.md
```

---

## Template

```markdown
# AGENTS.md

## Project Overview
<!-- 2-3 sentences. What is this? What's the tech stack? -->

## Quick Start
<!-- Essential commands only -->
- **Install**: `...`
- **Test**: `...`
- **Lint**: `...`

## Project Structure
<!-- Key directories and their purpose -->

## ⚠️ Known Pitfalls
<!-- Things that repeatedly cause issues -->

## ✅ Reference Implementations
<!-- Pointers to "gold standard" code -->

## 📚 Additional Documentation
<!-- Index to deeper docs when needed -->
- Architecture: `docs/architecture.md`
- API Design: `docs/api-conventions.md`
```

---

## Checklist

Before finalizing your `AGENTS.md`:

- [ ] Line count < 300 (ideally < 100)
- [ ] Only project-specific, universally-applicable content
- [ ] Commands use `just` or unified interface (SSoT)
- [ ] No duplicate information from linters/formatters
- [ ] References use `file:line` pointers, not copied code
- [ ] Known pitfalls section includes actual recurring issues
- [ ] All referenced files/paths are valid

---

## Resources

### references/
- `first-principles.md`: Deep dive into LLM mechanics that influence these guidelines
- `examples/`: Sample AGENTS.md files for different project types
