#!/usr/bin/env bash
set -euo pipefail

mkdir -p plans/load-test-results

stamp="$(date +%F-%H%M%S)"
api_url="${API_URL:-https://drafthouse-api.tanmayep.dev}"
ui_url="${UI_URL:-https://drafthouse.tanmayep.dev}"

: "${LOADTEST_EMAIL:?Set LOADTEST_EMAIL to a verified seeded user}"
: "${LOADTEST_PASSWORD:?Set LOADTEST_PASSWORD}"

run_k6() {
  local name="$1"
  local script="$2"
  local vus="$3"
  local duration="$4"
  local summary="plans/load-test-results/${stamp}-${name}-summary.json"
  local log="plans/load-test-results/${stamp}-${name}.txt"

  echo "==> ${name}: ${vus} VUs for ${duration}"
  set +e
  API_URL="$api_url" VUS="$vus" DURATION="$duration" \
    k6 run --summary-export "$summary" "$script" | tee "$log"
  local status="${PIPESTATUS[0]}"
  set -e
  return "$status"
}

http_status=0
ws_status=0
run_k6 http-smoke load-tests/http.js "${HTTP_SMOKE_VUS:-10}" "${HTTP_SMOKE_DURATION:-5m}" || http_status=$?
run_k6 ws-smoke load-tests/ws.js "${WS_SMOKE_VUS:-10}" "${WS_SMOKE_DURATION:-5m}" || ws_status=$?

if [[ "${RUN_PLAYWRIGHT:-0}" == "1" ]]; then
  echo "==> playwright smoke"
  (cd frontend && PLAYWRIGHT_BASE_URL="$ui_url" pnpm playwright test) \
    | tee "plans/load-test-results/${stamp}-playwright.txt"
fi

cat > "plans/load-test-results/${stamp}-notes.md" <<EOF
# Load test ${stamp}

- API_URL: ${api_url}
- UI_URL: ${ui_url}
- HTTP smoke: ${HTTP_SMOKE_VUS:-10} VUs for ${HTTP_SMOKE_DURATION:-5m}
- WS smoke: ${WS_SMOKE_VUS:-10} VUs for ${WS_SMOKE_DURATION:-5m}
- Playwright: ${RUN_PLAYWRIGHT:-0}
- HTTP status: ${http_status}
- WS status: ${ws_status}

## Result

TODO: record pass/fail, bottleneck, and next plateau.
EOF

echo "Results stored in plans/load-test-results/${stamp}-*"
if (( http_status != 0 || ws_status != 0 )); then
  exit 1
fi
