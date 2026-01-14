# Contributing to this Agent-Optimized Repository

This repository is optimized for both human developers and autonomous AI coding agents. We follow strict quality gates and a plan-first development workflow.

## Development Setup

1. **Install Bun**: Follow instructions at [bun.sh](https://bun.sh).
2. **Install Just**: [Just](https://github.com/casey/just) is used as our task runner.
3. **Setup Environment**:
   ```bash
   just install
   ```

## Workflow (Autonomous Agent Protocols)

We strictly follow the protocols defined in [AGENTS.md](./AGENTS.md):

1. **Plan First**: Never code without a task marked in your plan.
2. **TDD**: Write failing tests before implementation.
3. **QA**: Run `just qa` before every commit.
4. **Checkpoint**: Create descriptive commits with verification notes.

## Quality Gates

Before submitting a PR, ensure all checks pass:

```bash
just qa
```

This runs:
- **Linting**: ESLint + Prettier check.
- **Type-Check**: TypeScript compiler validation.
- **Testing**: Bun's native test runner.

## Commit Guidelines

We use [Conventional Commits](https://www.conventionalcommits.org/):
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `chore`: Maintenance
- `test`: Adding/modifying tests
