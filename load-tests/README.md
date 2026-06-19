# Load tests

Uses `k6`. Results are written to `plans/load-test-results/`.

## Required env

```bash
export LOADTEST_EMAIL='verified-user@example.com'
export LOADTEST_PASSWORD='password123'
```

Optional:

```bash
export API_URL='https://drafthouse-api.tanmayep.dev'
export UI_URL='https://drafthouse.tanmayep.dev'
```

## Smoke run

```bash
./load-tests/run.sh
```

Creates:

```text
plans/load-test-results/YYYY-MM-DD-HHMMSS-http-smoke-summary.json
plans/load-test-results/YYYY-MM-DD-HHMMSS-http-smoke.txt
plans/load-test-results/YYYY-MM-DD-HHMMSS-ws-smoke-summary.json
plans/load-test-results/YYYY-MM-DD-HHMMSS-ws-smoke.txt
plans/load-test-results/YYYY-MM-DD-HHMMSS-notes.md
```

## Find current capacity

Run plateaus until thresholds fail:

```bash
HTTP_SMOKE_VUS=25 WS_SMOKE_VUS=25 ./load-tests/run.sh
HTTP_SMOKE_VUS=50 WS_SMOKE_VUS=50 ./load-tests/run.sh
HTTP_SMOKE_VUS=100 WS_SMOKE_VUS=100 ./load-tests/run.sh
```

Capacity = highest passing plateau. Safe production limit = roughly 70% of that.

## UI smoke during load

```bash
RUN_PLAYWRIGHT=1 ./load-tests/run.sh
```

## Notes

- `http.js` simulates logged-in API usage: create/list/get/patch docs, content, and WS ticket.
- `ws.js` opens one hot document and connects VUs over WebSocket.
- The WS script tests connection capacity, not real Yjs editing payloads.
