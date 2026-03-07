# Agentic Native Repository Structure

This document outlines the **First Principles** for structuring a repository to be "Agentic Native"—optimized for AI agents to navigate, understand, and modify.

## 1. The Monorepo Structure: "Apps vs. Packages"

We strictly adhere to a **Unified "Apps & Packages" Structure**. This separates *deployable artifacts* from *consumable libraries*, regardless of the programming language.

### Ideal Directory Layout

```text
my-monorepo/
├── apps/                      # 🚀 Deployable Binaries & Servers
│   ├── web-app/               # (e.g., Next.js, React)
│   ├── api-gateway/           # (e.g., Rust binary, Go service)
│   └── background-worker/     # (e.g., Python script)
│
├── packages/                  # 📦 Shared Libraries (Internal Dependencies)
│   ├── ui/                    # Shared UI Components
│   ├── core-logic/            # Shared Business Logic (Rust lib, Python pkg)
│   ├── database/              # Schema & ORM clients
│   └── utils/                 # Helper functions
│
├── tools/                     # 🛠️ Custom Build Tools & Scripts
├── docs/                      # 📚 Global Documentation (Architecture, Guides)
├── .agent/                    # 🤖 Agent Context & Skills
│   ├── rules/                 # Context-specific rules
│   └── skills/                # Specialized tools/scripts
├── justfile                   # ⚡ Unified Command Runner
└── [Lockfiles & Configs]
```

### Key Rules
*   **No Language Silos**: Do not create generic `/rust` or `/node` folders. Group by logic (apps/packages), not syntax.
*   **Apps**: Must contain an entry point (`main.rs`, `index.js`, `main.py`). These *consume* packages.
*   **Packages**: Must be libraries. These *are consumed* by apps or other packages.

---

## 2. Feature-First Organization (Inside Modules)

Inside any application or package, we organize by **Feature**, not file type.

### ❌ The Anti-Pattern (File Type)
```text
src/
  ├── controllers/
  ├── models/
  └── utils/
```
*Why it fails*: Agents must context-switch across multiple directories to understand one feature.

### ✅ The Standard (Feature-First)
```text
src/
  ├── features/
  │   ├── auth/
  │   │   ├── components/
  │   │   ├── api.ts
  │   │   ├── types.ts
  │   │   └── README.md
  │   └── billing/
  │       ├── ...
  ├── shared/
  └── main.ts
```

---

## 3. The Triad of Self-Documenting Modules

Every major feature or module must adhere to the "Triad" structure to be fully self-contained for an agent.

| Artifact | Role | Principle | Expectation |
| :--- | :--- | :--- | :--- |
| **README.md** | **Context** | "What is this?" | Scope, Boundaries, Identity. Must exist at the root of the feature folder. |
| **Source Code** | **Logic** | "How it works" | Clean code, minimal "explanation" comments. |
| **Tests** (*_test.go, *.test.ts) | **Usage** | "How to use it" | Verified examples of usage patterns. |

### The "Local README" Rule
An agent entering a folder `src/features/auth/` should immediately see a `README.md` that explains:
1.  **Purpose**: What this module handles (and what it doesn't).
2.  **Key Components**: High-level map of the files.
3.  **Dependencies**: What it relies on.

---

## 4. Documentation Strategy

*   **`docs/` (Root)**: Organized via the **Diátaxis Framework**:
    *   `tutorials/`: Learning-oriented lessons.
    *   `guides/`: Problem-oriented "How-to" guides.
    *   `reference/`: Information-oriented specs (API, CLI).
    *   `explanations/`: Understanding-oriented background & architecture.
*   **`apps/docs-site/`**: The deployed documentation website (e.g., Docusaurus). This is the *product* for users.
*   **`README.md` (root)**: The "Landing Page". Instructions on how to bootstrap the repo.

---

## 5. Type and Schema Organization

We distinguish between **Domain Types** (local) and **Shared Contracts** (global).

### Domain-Specific Types
*   **Rule**: If a type is primarily used by one feature, it belongs **inside that feature**.
*   **Location**: `src/features/auth/types.ts`
*   **Rationale**: Changing logic often requires changing types. Co-location reduces context switching.

### Shared Contracts (API Compatibility)
*   **Rule**: If a type defines the protocol between two distinct parts (e.g., Backend <-> Frontend), it belongs in a shared package.
*   **Location**: `packages/schema/` or `packages/contract/`.
*   **Rationale**: Ensures Producer and Consumer use the absolute same version of the truth.

### Auto-Generation (Browsability)
*   Prefer using tools that generate unified documentation from these distributed types.
    *   **TypeScript**: `TypeDoc`
    *   **Rust**: `cargo doc --workspace`
    *   **API**: `TSOA` or `Utoipa` (Code-First OpenAPI generation)

---

## 6. Unified Command Interface (`justfile`)

Use `just` as the single source of truth for all commands.
*   **Goal**: An agent should never guess between `npm run dev`, `cargo run`, or `make start`.
*   **Standard**: Always provide `just dev`, `just build`, `just test`.

---

## 7. Test Organization & Distribution

Testing is a primary navigation tool for agents. We organize tests by **scope** and **intent** to ensure agents can verify their work at the appropriate level.

### The Testing Pyramid (Agentic View)

| Test Layer | Config/Location | Purpose | Agentic Principle |
| :--- | :--- | :--- | :--- |
| **Linting & Formatting** | `tools/configs/` | Enforce style, prevent syntax errors. | **"The bouncer"**. Fails fast on syntax/style before logic is checked. |
| **Doc Tests** | Inside Source comments | Verify examples in documentation. | **"Truth in Advertising"**. Ensures explanations match reality. |
| **Unit Tests** | Co-located `*_test.*` | Verify isolated functions/classes. | **"The Specification"**. Define behavior of atomic units. |
| **Logic/BDD Tests** | `tests/logic/` or `*.spec.*` | Verify business rules in human-readable format. | **"The Business Contract"**. Connects requirements to implementation. |
| **Integration/E2E** | `tests/e2e/` (Root) or `apps/*/tests/` | Verify full system flow. | **"The User Journey"**. Validates the complete experience. |

### Detailed Organization Rules

#### 1. Linting & Static Analysis (Config in Tools)
*   **Rule**: Centralize configurations to ensure consistency.
*   **Location**: `tools/configs/` (e.g., `tools/configs/eslint-preset.js`).
*   **Usage**: Packages/Apps extend these base configs. *Note: Root-level config files (like `.prettierrc.js`) are acceptable ONLY if they just import/extend the shared config from `tools/`.*

#### 2. Documentation Tests (DocTests)
*   **Rule**: Code examples in docstrings or READMEs must be executable/verifiable.
*   **Tools**: Rust (`cargo test --doc`), Python (`doctest`), JS/TS (Vitest type-checking).
*   **Rationale**: Prevents "rotten" documentation which misleads agents.

#### 3. Unit Tests (Co-located)
*   **Rule**: Keep unit tests strictly adjacent to the file they test.
*   **Location**: `src/utils/math.ts` → `src/utils/math.test.ts`.
*   **Context**: Allows agents to see Implementation + Verification in a single directory view (The Triad).

#### 4. Business Logic Specifications (BDD)
*   **Rule**: Use Behavior-Driven Development (BDD) style for complex business logic.
*   **Naming**: `*.spec.ts` or `specs/*.feature`.
*   **Style**: Use `Describe`, `It`, or `Given/When/Then` syntax.
*   **Focus**: Test the *outcome* of business rules, not the implementation details.

#### 5. Integration & E2E Tests
*   **Rule**: Treat E2E tests as a separate "Application" or "Client" that consumes your App.
*   **Location**:
    *   **Global**: `tests/e2e/` (at repo root) for cross-service workflows.
    *   **App-Specific**: `apps/<app-name>/e2e/` for isolated app flows.
*   **Separation**: Keep these out of `src/` to prevent circular dependencies.

### Supplements

#### Test Fixtures & Factories
*   **Rule**: Do not hardcode complex data in tests. Use factories.
*   **Location**: `packages/testing/factories/` or `tests/fixtures/`.
*   **Rationale**: Keeps test logic clean and readable.

#### Snapshot Testing
*   **Rule**: Use sparingly. Best for "Output Stability" (e.g. generated config files, CLI output), not "Logic Verification".

---

## 8. Documentation & Commenting Architecture

Structure is not just folders; it's how knowledge is embedded in code. We follow the **Rustdoc Philosophy** where documentation is structured, locatable, and verifiable.

### The Hierarchy of Knowledge

| knowledge Layer | Location | Form | Purpose |
| :--- | :--- | :--- | :--- |
| **Logic** | The Code itself | Types, Function Names | Defines *WHAT* happens. Must be self-documenting. |
| **Flow** | Code Comments | BDD (`// given`, `// when`, `// then`) | Defines *WHY* logic flows a certain way. |
| **Usage** | Test Files | Unit Tests, Examples | Shows *HOW* to use it. verified by CI. |
| **Context** | `README.md` | Markdown | Defines *System Boundaries* and High-Level Design. |
| **Decisions** | `docs/adr/` | ADRs (Markdown) | Explains *History* and Trade-offs (Why this, not that?). |

### Core Rules

#### 1. Prioritize Types Over Comments
*   **Anti-Pattern**: `// User data with name and age` `struct Data { ... }`
*   **Standard**: `struct UserProfile { name: string, age: number }`
*   **Rule**: If a comment explains *what* a variable is, rename the variable instead.

#### 2. The "No Memo" Policy
*   **Anti-Pattern**: `// Changed this fix bug #123`
*   **Standard**: Use Git Commits for history.
*   **Rule**: Code files are for the *current state*, not the *history*.

#### 3. BDD-Style Inline Comments
*   **Rule**: Use comments to divide complex logic into "Chapters":
    ```typescript
    // 1. Validate Input
    if (!input.isValid) return;

    // 2. Transform Data
    const data = transform(input);

    // 3. Persist
    await db.save(data);
    ```

#### 4. Architecture Pointers
*   **Rule**: When code looks "weird" due to an architectural decision, link to the ADR.
*   **Format**: `// See ADR-005 for why we use a global lock here.`

