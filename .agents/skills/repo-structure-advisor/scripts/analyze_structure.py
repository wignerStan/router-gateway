#!/usr/bin/env python3
import os
import sys
from pathlib import Path

# Colors for output
GREEN = "\033[92m"
YELLOW = "\033[93m"
RED = "\033[91m"
RESET = "\033[0m"


def log_pass(msg):
    print(f"{GREEN}[PASS] {msg}{RESET}")


def log_warn(msg):
    print(f"{YELLOW}[WARN] {msg}{RESET}")


def log_fail(msg):
    print(f"{RED}[FAIL] {msg}{RESET}")


def check_monorepo_root(root: Path):
    print(f"🔍 Analyzing structure at: {root}\n")

    # 1. Apps & Packages
    has_apps = (root / "apps").is_dir()
    has_packages = (root / "packages").is_dir()

    if has_apps and has_packages:
        log_pass("Found 'apps/' and 'packages/' directories (Unified Structure).")
    elif has_apps:
        log_warn(
            "Found 'apps/' but missing 'packages/'. Consider separating shared logic."
        )
    else:
        log_fail(
            "Missing 'apps/' directory. Agentic Native structure prefers separating deployables into 'apps/'."
        )

    # 2. Key Config Files (Polyglot check)
    configs = {
        "justfile": "Unified Command Interface",
        "pnpm-workspace.yaml": "Node Workspace Config",
        "Cargo.toml": "Rust Workspace Config",
        ".agent": "Agent Context Directory",
    }

    for filename, distinct in configs.items():
        if (root / filename).exists():
            log_pass(f"Found {filename} ({distinct}).")
        else:
            if filename == ".agent":
                log_fail(f"Missing {filename}. This is critical for agentic workflows.")
            else:
                log_warn(f"Missing {filename}. Recommended for {distinct}.")

    # 3. Documentation
    if (root / "docs").is_dir():
        log_pass("Found 'docs/' directory for Central Documentation.")
    else:
        log_warn("Missing 'docs/' directory. Where is the architecture documented?")


def check_feature_structure(root: Path):
    # Heuristic: verify if there are generic folders that should be features
    # This is a bit looser, just checking for common anti-patterns
    anti_patterns = ["controllers", "models", "views", "utils"]

    found_anti_patterns = []
    # Also check for root types folder (Anti-Pattern: "Junk Drawer")
    if (root / "types").is_dir():
        found_anti_patterns.append(
            "types (Root level - prefer co-location or packages/schema)"
        )

    for root_dir, dirs, files in os.walk(root):
        if ".git" in root_dir or "node_modules" in root_dir or ".agent" in root_dir:
            continue

        path = Path(root_dir)
        # Check direct children of src if it exists, or just root children
        for ap in anti_patterns:
            if ap in dirs:
                rel_path = path / ap
                found_anti_patterns.append(str(rel_path.relative_to(root)))

    if found_anti_patterns:
        print(
            "\n⚠️  Possible 'File Type' Anti-Patterns found (Consider grouping by Feature instead):"
        )
        for p in found_anti_patterns[:5]:  # Limit output
            print(f"   - {p}")
        if len(found_anti_patterns) > 5:
            print(f"   ... and {len(found_anti_patterns) - 5} more.")
    else:
        log_pass("No obvious 'controllers/models' anti-patterns found.")


def main():
    root_path = Path(os.getcwd())
    if len(sys.argv) > 1:
        root_path = Path(sys.argv[1])

    if not root_path.exists():
        print(f"Error: Path {root_path} does not exist.")
        sys.exit(1)

    check_monorepo_root(root_path)
    check_feature_structure(root_path)

    print(
        "\n💡 Tip: Run 'view_file .agent/skills/repo-structure-advisor/references/agentic_structure.md' for the detailed spec."
    )


if __name__ == "__main__":
    main()
