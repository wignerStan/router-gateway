# TypeScript Workspace Template

A modern, production-ready TypeScript workspace template with multiple packages, based on best practices from production projects.

## Features

- **Workspace architecture** - Centralized dependency management across packages
- **Monorepo structure** - Support for frontend, backend, and shared packages
- **Modern CI/CD** - GitHub Actions with caching and parallel jobs
- **Comprehensive tooling** - ESLint, Prettier, TypeScript strict mode
- **Docker support** - Multi-stage builds with security best practices
- **AI-friendly rules** - Clear guidelines for AI-assisted development
- **Package manager options** - Supports both Bun and pnpm
- **Security-first** - gitleaks secret scanning, dependency audit
- **Pre-commit hooks** - Automated quality checks before commits
- **Just task runner** - Comprehensive task automation

## Quick Start

```bash
# Initialize a new workspace
mkdir my_workspace && cd my_workspace

# Copy workspace template
cp -r ../bun-template/* .

# Replace placeholders
find . -name "package.json" -exec sed -i "" 's/{{PROJECT_NAME}}/my_project/g' {} \;
find . -name "package.json" -exec sed -i "" 's/{{PROJECT_DESCRIPTION}}/My TypeScript Project/g' {} \;
find . -name "package.json" -exec sed -i "" 's/{{AUTHOR_NAME}}/Your Name/g' {} \;

# Install dependencies
bun install

# Build and test
bun run build
bun run test
```

## Project Structure

```
.
├── packages/                 # Workspace members
│   ├── core/                # Core library package
│   ├── api/                 # API package (if applicable)
│   ├── bin/                 # Binary/CLI package
│   ├── frontend/             # Optional: Frontend workspace
│   └── backend/             # Optional: Backend workspace
├── package.json             # Workspace configuration
├── bun.lockb               # Dependency lock file
├── tsconfig.json            # Root TypeScript configuration
├── tsconfig.base.json       # Base TypeScript configuration (reference)
├── .pre-commit-config.yaml  # Pre-commit hooks
├── justfile                # Task runner
├── AGENTS.md               # AI and human coding guidelines
├── Dockerfile.template      # Multi-stage Docker build
├── .dockerignore           # Docker ignore patterns
├── .gitignore              # Git ignore patterns
└── README.md               # This file
```

## Workspace Configuration

The root `package.json` defines workspace and shared scripts:

```json
{
  "name": "{{PROJECT_NAME}}-workspace",
  "version": "1.0.0",
  "private": true,
  "workspaces": [
    "packages/*"
  ],
  "scripts": {
    "dev": "bun run --filter '*' dev",
    "build": "bun run --filter '*' build",
    "test": "bun run --filter '*' test",
    "lint": "bun run --filter '*' lint",
    "type-check": "bun run --filter '*' type-check"
  }
}
```

Individual packages use workspace dependencies:

```json
{
  "dependencies": {
    "@{{PROJECT_SCOPE}}/{{PROJECT_NAME}}-core": "workspace:*"
  }
}
```

## Development Workflow

### 1. Code

```bash
# Build entire workspace
bun run build

# Build specific package
cd packages/core && bun run build

# Run tests for workspace
bun run test

# Run tests for specific package
cd packages/api && bun test
```

### 2. Format and Lint

```bash
# Format all workspace code
just fmt

# Check formatting
just fmt-check

# Run linter
just lint

# Type check
just type-check
```

### 3. Build for Production

```bash
# Optimized build
bun run build

# Run production binary
bun packages/bin/dist/index.js
```

### 4. Using Just

```bash
# Build workspace
just build

# Run all tests
just test

# Run all quality checks
just qa

# Format code
just fmt

# Run linter
just lint

# Type check
just type-check
```

## Package Organization

### Core Package (`packages/core`)
- Shared types and utilities
- Domain models
- Business logic
- No external dependencies beyond workspace

### API Package (`packages/api`)
- HTTP handlers (if using a web framework)
- API types and schemas
- Request/response models
- Depends on `core`

### Binary Package (`packages/bin`)
- Application entry point
- CLI argument parsing
- Configuration loading
- Depends on `core` and `api`

### Frontend Package (`packages/frontend`, optional)
- UI components
- State management
- Routing
- Depends on `api` and `core`

### Backend Package (`packages/backend`, optional)
- Server setup
- API routes
- Middleware
- Depends on `api` and `core`

## CI/CD

The GitHub Actions workflows include:

**CI Pipeline** (`.github/workflows/ci.yml`):
- **Test** - Runs tests on multiple platforms and Node versions
- **Lint** - Runs ESLint with strict warnings
- **Build** - Verifies workspace builds successfully
- **Security Audit** - Runs gitleaks for secret scanning
- **Gitleaks** - Secret detection
- **Documentation** - Builds documentation

**Security Pipeline** (`.github/workflows/security.yml`):
- **Gitleaks** - Secret scanning on every push
- **Audit** - Dependency vulnerability checks
- **Dependency Review** - Reviews dependencies for security issues

## Pre-commit Hooks

The `.pre-commit-config.yaml` includes hooks for:
- Trailing whitespace and end-of-file fixes
- YAML, JSON, and TOML validation
- Large file detection
- Merge conflict detection
- Private key detection
- **gitleaks** - Secret scanning (replaces detect-secrets)
- Prettier formatting
- Lint check
- Type check
- Test check

## Security

This template includes comprehensive security tools:

- **gitleaks** - Secret scanning in pre-commit and CI
- **dependency audit** - Vulnerability scanning
- **secrecy equivalents** - Secure handling of environment variables
- **Dependency Review** - Automated dependency security review

## AI-Assisted Development

This template includes `AGENTS.md` with guidelines for AI assistants:

- No unsafe patterns or side effects unless explicitly asked
- Prefer immutability and functional patterns
- Follow TypeScript best practices (strict mode, no implicit any)
- Use typed error classes or Result types
- Provide meaningful context with error messages

## Adding a New Package

```bash
# Create new package directory
mkdir packages/new_package

# Create package.json
cat > packages/new_package/package.json << 'EOF'
{
  "name": "@{{PROJECT_SCOPE}}/{{PROJECT_NAME}}-new_package",
  "version": "1.0.0",
  "description": "Description of new package",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "type": "module",
  "scripts": {
    "build": "tsc --build",
    "dev": "tsc --watch",
    "test": "bun test",
    "lint": "eslint src",
    "lint:fix": "eslint src --fix",
    "type-check": "tsc --noEmit"
  }
}
EOF

# Add workspace dependency if needed
# Add to tsconfig.json references
```

## Docker

```bash
# Build Docker image
docker build -t my_project .

# Run container
docker run -p 3000:3000 my_project
```

## License

MIT
