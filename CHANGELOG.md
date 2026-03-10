# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial template structure
- Workspace configuration for Bun package manager
- TypeScript project references support
- Pre-commit hooks with gitleaks integration
- GitHub Actions CI/CD workflows

### Changed
- Removed detect-secrets in favor of gitleaks
- Enhanced TypeScript configuration with strict mode options

### Security
- gitleaks for secret scanning
- Dependency audit workflow

---

## [1.0.0] - YYYY-MM-DD

### Initial Release
- Workspace architecture with packages/core, packages/api, packages/bin
- Comprehensive documentation (AGENTS.md, README-WORKSPACE.md, ROADMAP.md)
- Production-ready CI/CD pipeline
- Docker support with multi-stage builds
- Just task runner with comprehensive commands
- Security-first configuration (gitleaks, audit)
