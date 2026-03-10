# Example AGENTS.md Files

## Python API Project (FastAPI)

```markdown
# AGENTS.md

## Project Overview
A REST API for user management built with FastAPI, SQLAlchemy, and PostgreSQL.
Python 3.12+ with strict type checking via mypy.

## Quick Start
| Task | Command |
|------|---------|
| Install | `uv pip install -e ".[dev]"` |
| Test | `just test` |
| Full QA | `just qa-full` |
| Dev Server | `just dev` |
| Format | `just fmt` |

## Project Structure
- `src/api/` — Route handlers (one file per resource)
- `src/models/` — SQLAlchemy models
- `src/schemas/` — Pydantic request/response schemas
- `src/services/` — Business logic (keep routes thin)
- `tests/` — Mirrors `src/` structure

## ⚠️ Known Pitfalls
- All DB operations are async — always `await` session methods
- Use `SessionDep` injection, never create sessions directly
- Pydantic v2 syntax only (`model_validate`, not `parse_obj`)

## ✅ Reference Implementations
- CRUD endpoint: `src/api/users.py`
- Service pattern: `src/services/auth_service.py`
- Integration test: `tests/integration/test_users.py`
```

---

## Node.js Monorepo (pnpm + Turborepo)

```markdown
# AGENTS.md

## Project Overview
Monorepo with 3 apps: `web` (Next.js), `api` (Express), `admin` (Remix).
Shared packages in `packages/`. Uses pnpm workspaces + turborepo.

## Quick Start
| Task | Command |
|------|---------|
| Install | `pnpm install` |
| Dev (all) | `pnpm dev` |
| Dev (one) | `pnpm --filter web dev` |
| Test | `just test` |
| Build | `just build` |

## Project Structure
- `apps/web/` — Public-facing Next.js app
- `apps/api/` — REST API backend
- `apps/admin/` — Admin dashboard (Remix)
- `packages/ui/` — Shared React components
- `packages/db/` — Prisma schema + client

## ⚠️ Known Pitfalls
- NEVER use `npm` or `yarn` — pnpm only
- Import shared packages via `@repo/package-name`
- Turborepo caching: delete `.turbo/` if builds seem stale

## 🤖 Agent Guidelines
- When modifying shared packages, test ALL consuming apps
- Prefer editing one app at a time per session
- Run `pnpm --filter <app> typecheck` before committing
```

---

## Rust CLI Tool

```markdown
# AGENTS.md

## Project Overview
A CLI tool for file processing. Rust 1.75+, uses clap for args, tokio for async.

## Quick Start
| Task | Command |
|------|---------|
| Build | `cargo build` |
| Test | `just test` |
| Run | `cargo run -- <args>` |
| Lint | `just lint` |

## ⚠️ Known Pitfalls
- Use `anyhow::Result` for error propagation
- Prefer `PathBuf` over `String` for file paths
- All public items need doc comments (`///`)

## ✅ Reference Implementations
- CLI setup: `src/main.rs` (clap derive pattern)
- Error handling: `src/errors.rs`
- Async file ops: `src/fs/reader.rs`
```

---

## Minimal Example (~30 lines)

```markdown
# AGENTS.md

A static site generator using Eleventy. Node 20+, pnpm.

## Commands
- Install: `pnpm install`
- Dev: `pnpm dev` (starts local server on :8080)
- Build: `pnpm build` (outputs to `_site/`)

## Structure
- `src/` — Source templates (Nunjucks)
- `src/_data/` — Global data files
- `_site/` — Build output (gitignored)

## Notes
- Images go in `src/assets/img/` (auto-optimized on build)
- Add new pages in `src/pages/`
```
