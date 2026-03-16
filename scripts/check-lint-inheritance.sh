#!/usr/bin/env bash
# Check that all workspace crates inherit [lints] from the workspace root.
# Usage: ./scripts/check-lint-inheritance.sh
# Exit 0 if all crates inherit, exit 1 otherwise.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Checking workspace members for [lints] workspace = true ..."
echo ""

missing=0
total=0

while IFS= read -r manifest; do
    pkg=$(grep '^name = ' "$manifest" | head -1 | cut -d'"' -f2)
    has_lints=$(grep -c '^\[lints\]' "$manifest" 2>/dev/null || true)
    has_ws=$(grep -A1 '^\[lints\]' "$manifest" 2>/dev/null | grep -c 'workspace = true' || true)

    total=$((total + 1))

    if [ "$has_lints" -eq 0 ] || [ "$has_ws" -eq 0 ]; then
        echo "  ✗ $pkg ($manifest)"
        missing=$((missing + 1))
    else
        echo "  ✓ $pkg"
    fi
done < <(find "$ROOT_DIR/cli" "$ROOT_DIR/crates" -name Cargo.toml -maxdepth 2 2>/dev/null)

echo ""

if [ "$missing" -gt 0 ]; then
    echo "⚠️  $missing of $total crate(s) do not inherit workspace lints"
    echo "Add the following to each missing Cargo.toml:"
    echo ""
    echo "  [lints]"
    echo "  workspace = true"
    exit 1
fi

echo "✅ All $total workspace members inherit workspace lints"
