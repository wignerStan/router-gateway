# Agent Workbench Protocols

## 1. Plan-First Development (The Source of Truth)

- **Principle:** All work MUST be tracked in `task.md` or a project `plan.md`.
- **Workflow:**
  1. Select the next task in sequential order.
  2. Mark the task as "In Progress" `[/]` before beginning.
  3. Update the task to "Complete" `[x]` only after verification.
- **Verification:** Always cite the implementation plan or task list when justifying changes.

## 2. Test-Driven Development (Red-Green-Refactor)

- **Red:** Write a failing unit or integration test that defines the expected behavior.
- **Green:** Implement the minimum code necessary to make the test pass.
- **Refactor:** Clean up the code while ensuring the tests remain green.
- **Coverage:** Aim for >80% code coverage for all new modules.

## 3. Config is Code

- **Principle:** All behavioral changes must be defined in configuration files (`config/` or `.config/`).
- **Forbidden:** Do not hardcode rules, permissions, or workflow logic in the source code.
- **Verification:** Changes to config must be verified via `aw config reload` (or project equivalent).

## 4. Hook System Usage

- **Definition:** Use hooks to extend functionality without modifying the core.
- **Workflow:** Define trigger -> Define action -> Test manually or via `just test`.

## 5. Non-Interactive & CI-Aware

- **Constraint:** Prefer non-interactive commands.
- **Environment:** Use `CI=true` for watch-mode tools to ensure single-pass execution in automation.

## 6. Verification & Checkpointing

- **Protocol:** After completing a significant phase or task:
  1. Run the full QA suite: `just qa`.
  2. Perform manual verification based on the product goals.
  3. Create a checkpoint commit describing the state.
- **Reporting:** Attach an auditable verification report (notes) to the checkpoint commit.

# Project Rules for AI and Humans

## General

- Do **not** introduce `unsafe` patterns or side effects unless explicitly asked.
- Prefer immutability and functional patterns where appropriate.
- Follow TypeScript best practices (strict mode, no implicit any).
- Follow standard naming conventions (camelCase for variables/functions, PascalCase for classes/types).

## Error Handling

- Use typed error classes or Result types.
- Provide meaningful context with error messages.

## AI Usage

- You (AI) are a Senior System Engineer.
- Always ensure generated code is idiomatic and lint-free.
- **Red-Green-Refactor** is your default mode of operation.
- Before writing code, briefly outline your plan in comments.

## Git Workflow

- Write meaningful commit messages following Conventional Commits.
- Run `just qa` before committing.
