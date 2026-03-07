# Justfile Syntax Guide

This reference covers the essential `justfile` syntax patterns needed for the `justfile-optimizer` skill.

## 1. Recipes (Commands)

Recipes are the core building blocks. They are like makefile targets but simpler.

```just
# A simple recipe
test:
    echo "Running tests..."
    npm test

# Recipe with dependencies (will run 'build' first)
serve: build
    npm start
```

## 2. Dependencies and Execution Order

`just` executes dependencies *before* the recipe itself.

```just
# 'fmt', 'lint', and 'test-fast' run in order before 'qa'
qa: fmt lint test-fast
    echo "QA complete!"
```

## 3. Variables

Variables allow for reuse and configuration.

```just
# Define variable
node_version := "18"

# Use variable
install:
    echo "Installing for Node {{node_version}}"
    npm install
```

## 4. Cross-Platform Compatibility

Use `env` to set environment variables per recipe line, or export them globally.

```just
# Export variable for all child processes
export NODE_ENV := "production"

# Use a specific shell (e.g., bash) for consistency
set shell := ["bash", "-c"]

test:
    # Cross-platform way to set env var for one command
    env CI=true npm test
```

## 5. Command Evaluation

You can assign the output of a command to a variable using backticks.

```just
# Get git commit hash
git_hash := `git rev-parse --short HEAD`

version:
    echo "Build version: {{git_hash}}"
```

## 6. Default Recipe

The first recipe is the default (run by just typing `just`). It is good practice to make this `list` or `help`.

```just
default:
    just --list
```
