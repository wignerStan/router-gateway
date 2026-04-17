#!/usr/bin/env bash
# scripts/check-branch-coverage.sh
# Parses cargo llvm-cov --branch --json output and checks branch coverage threshold.
# Usage: ./scripts/check-branch-coverage.sh <threshold>
# Example: ./scripts/check-branch-coverage.sh 70

set -euo pipefail

THRESHOLD="${1:-70}"
IGNORED_FILES="src/main\.rs|src/bin/cli\.rs"
COVERAGE_JSON=$(mktemp)

# Generate branch coverage as JSON
cargo llvm-cov --branch --json --output-path "$COVERAGE_JSON" \
  --ignore-filename-regex "$IGNORED_FILES" 2>/dev/null

# Extract branch coverage percentage from the JSON summary
BRANCH_PCT=$(python3 -c "
import json, sys
with open('$COVERAGE_JSON') as f:
    data = json.load(f)
try:
    pct = data['data'][0]['totals']['branches']['percent']
except (KeyError, IndexError):
    pct = 0.0
print(f'{pct:.2f}')
")

rm -f "$COVERAGE_JSON"

echo "Branch coverage: ${BRANCH_PCT}% (threshold: ${THRESHOLD}%)"

PASS=$(python3 -c "print('true' if float('$BRANCH_PCT') >= float('$THRESHOLD') else 'false')")

if [ "$PASS" != "true" ]; then
    echo "FAIL: Branch coverage ${BRANCH_PCT}% is below threshold ${THRESHOLD}%"
    exit 1
fi

echo "PASS: Branch coverage ${BRANCH_PCT}% meets threshold ${THRESHOLD}%"
