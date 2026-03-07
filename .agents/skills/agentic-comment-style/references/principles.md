# Code Comment Principles for the AI Era

## 1. The Fundamental Principle

> "Code is like humor. When you have to explain it, it's bad." — Cory House

Comments should not explain **WHAT** code does—the code itself should be clear enough. Comments serve only three legitimate purposes:
1.  **Machine Directives**: Instructions for tools (linters, compilers).
2.  **Contextual Why**: Business logic rationale that cannot be expressed in code.
3.  **Navigation Aids**: Structured markers (BDD sections) that help humans and AI navigate.

## 2. go-claude-code-comment-checker Philosophy

Based on the opinionated cleaner tool, we enforce specific validity rules:

### Allowed Categories
| Category | Example | Rationale |
|----------|---------|-----------|
| **BDD Keywords** | `# given`, `# when`, `# then`, `# arrange` | Test organization structure |
| **Linter Directives** | `# noqa`, `// @ts-ignore` | Functional machine instructions |
| **Shebangs** | `#!/usr/bin/env python` | Executable metadata |
| **Docs Pointers** | `See docs/auth.md` | Reduces context token waste |

### Explicitly Rejected
- **Agent Memos**: "Changed from sync to async", "Refactored this path". (Move to git commit).
- **Zombie Code**: Commented out code blocks. (Delete them).
- **Redundant Explanations**: "Increment counter" for `i++`.
- **Aspirational TODOs**: "Refactor later". (Do it now or file an issue).

## 3. AI-Specific Constraints

### Self-Documenting > Comments
AI reads code like natural language. Clear naming is better documentation than a comment block.
- **Bad**: `doStuff()` + comment
- **Good**: `processUserData()` (Zero tokens wasted on redundant comment)

### Error Messages as Documentation
Instead of commenting a throw statement, put the context *in* the error message.
- **Bad**: `// Input must be > 0`<br>`throw Error()`
- **Good**: `throw Error("Input must be > 0")`

### Naming Intuition
AI has statistical intuitions about naming from its training data.
- Use **conventional** names (`present()` vs `swapBuffers()`) to help the AI "guess" correctly.
- Do not fight the model's statistical probability with "clever" unconventional naming.

### Documentation Proximity
Place documentation *next* to the code (same directory).
- AI performs better when `README.md` is adjacent to `src/api/` rather than buried in a distant `docs/` folder.

## 4. The Triad of Self-Documenting Code

True documentation is distributed across three artifacts, each with a distinct role:

| Artifact | Role | Principle |
|----------|------|-----------|
| **The Code** | **Logic** | "How it works". Source of truth. Must remain clean. |
| **Local README.md** | **Context** | "What it is". Scope, Identity, TL;DR. **Links** to central docs. |
| **Central Docs** | **Insight** | "Why". Architecture, ADRs, Cross-cutting concerns. |
| **The Test** | **Usage** | "How to use". Verified examples. Corner cases. |

### The "Module README" Pattern
Every significant directory (module) should have a concise `README.md`.
```text
src/
  session/
    README.md    # "Handles Redis storage ops. See ADR-005 for Redis choice."
    manager.ts   # The implementation
    manager.test.ts # Proof it works
```
This keeps the directory self-describing without fragmenting the architectural "Brain" of the project.
