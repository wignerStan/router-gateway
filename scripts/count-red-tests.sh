#!/usr/bin/env bash
# Count red/edge vs total tests for ratio reporting.
set -euo pipefail

RED_FILTER='test(red_edge) + test(proptests) + test(routes_input_validation) + test(db_resilience) + test(concurrency_error_paths)'

TOTAL=$(cargo nextest list 2>/dev/null | grep -c '    ' || echo 0)
RED=$(cargo nextest list -E "${RED_FILTER}" 2>/dev/null | grep -c '    ' || echo 0)

if [ "${TOTAL}" -gt 0 ]; then
    RATIO=$(python3 -c "print(f'{${RED} / ${TOTAL} * 100:.1f}%')")
else
    RATIO="N/A"
fi

echo "Red/edge: ${RED}/${TOTAL} (${RATIO})"
