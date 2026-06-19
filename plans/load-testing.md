# Drafthouse Load Test Plan

Targets:
- API: `https://drafthouse-api.tanmayep.dev`
- UI: `https://drafthouse.tanmayep.dev`

## Goals

1. Find the safe concurrent-user limit before p95 latency or error rate degrades.
2. Validate the real-time editor limit: 100 concurrent editors per document should reject excess users cleanly with 429 / failed WS ticket or upgrade.
3. Catch obvious frontend regressions under backend load.

## What to measure

Minimum useful metrics:
- HTTP: p50/p95/p99 latency, requests/sec, non-2xx rate.
- WebSocket: connection success rate, message round-trip/broadcast latency, disconnects, reconnects.
- App: CPU, memory, DB connections, Postgres query latency, Scylla latency, container restarts.
- UI smoke: page load success, login flow success, editor opens and syncs.

Pass/fail starter thresholds:
- HTTP p95 < 500ms for common reads/writes.
- Error rate < 1% excluding intentional 429s.
- WS connection success > 99% up to expected capacity.
- No container restarts, OOMs, or unbounded memory growth.

## Scenarios

### 1. API smoke load

Small steady test to prove credentials, CORS, cookies, and scripts work.

Flow per virtual user:
1. Register unique user or login a seeded user.
2. `GET /auth/me`.
3. `POST /documents`.
4. `GET /documents`.
5. `GET /documents/{id}`.
6. `PATCH /documents/{id}`.
7. `GET /documents/{id}/content`.
8. `PATCH /documents/{id}/content`.
9. `POST /documents/{id}/ws-ticket`.

Run: 5 minutes, 5-10 VUs.

### 2. Normal API load

Same flow, weighted toward real usage:
- 50% list/get documents.
- 20% create/update metadata.
- 20% content reads/writes.
- 10% auth refresh/me/ws-ticket.

Run: 15 minutes at 25, 50, then 100 VUs.

### 3. Collaborative editing load

Use WebSocket clients against:

`wss://drafthouse-api.tanmayep.dev/collab/{doc_id}?ticket={ticket}`

Cases:
- Many docs: 10 editors each across 10 docs.
- Hot doc: ramp 1 shared document from 10 -> 100 editors.
- Limit test: attempt 110 editors on one document and verify only 100 are accepted, excess fail cleanly.

Each editor sends small Yjs-style update messages every 1-5 seconds and stays connected for 15 minutes.

### 4. Spike test

Ramp from 10 to 100 API VUs in 60 seconds, hold 5 minutes, ramp down.

Purpose: connection pool, autoscaling/restart behavior, DB saturation.

### 5. Soak test

Run normal API + WS load for 2 hours at the highest level that passed normal load.

Purpose: memory leaks in `DocStore`, stale WebSocket rooms, DB connection leaks, snapshot/WAL drift.

### 6. UI smoke during load

While scenarios 2-4 run, execute Playwright against `https://drafthouse.tanmayep.dev`:
1. Open home page.
2. Login.
3. Open document list.
4. Open editor.
5. Type text in two browser contexts and verify sync.

Run every 2-5 minutes, not as the main load generator.

## Tooling

Use the fewest tools:
- `k6` for HTTP and WebSocket load.
- Existing Playwright tests for UI smoke.
- Existing production metrics/logs for server and DB signals.

Do not use browser-based load for high concurrency; browsers are expensive and test the runner more than the app.

## Test data

Before load:
1. Create a dedicated load-test account namespace: emails like `loadtest+{uuid}@example.test`.
2. Seed 10-50 documents for read-heavy tests.
3. Seed one hot document for collaborative editor tests.
4. Disable or stub outbound email if registration triggers email provider limits.

After load:
1. Delete load-test users/documents if the app exposes cleanup.
2. Otherwise keep tests on unique prefixes so cleanup can be SQL-based.

## Execution order

1. Smoke API at 5-10 VUs.
2. Smoke WS with 2 clients on one doc.
3. Normal API ramp: 25 -> 50 -> 100 VUs.
4. Collaborative editing: many-docs, then hot-doc, then 110-editor cap test.
5. Spike test.
6. Soak test only after all above pass.

Stop immediately if:
- Error rate > 5% for 2 minutes.
- API p95 > 2s for 5 minutes.
- DB or app container restarts.
- CPU/memory stays saturated after ramp-down.

## Deliverables

- `load-tests/http.js`: k6 HTTP scenario.
- `load-tests/ws.js`: k6 WebSocket scenario.
- `load-tests/README.md`: exact commands and required env vars.
- One results note per run in `plans/load-test-results/YYYY-MM-DD.md`.

Required env vars for scripts:

```bash
API_URL=https://drafthouse-api.tanmayep.dev
UI_URL=https://drafthouse.tanmayep.dev
LOADTEST_EMAIL_PREFIX=loadtest
LOADTEST_PASSWORD='...'
```

## First command set

```bash
# API smoke
k6 run -e API_URL=https://drafthouse-api.tanmayep.dev load-tests/http.js

# WS smoke / collab
k6 run -e API_URL=https://drafthouse-api.tanmayep.dev load-tests/ws.js

# UI smoke while load is running
cd frontend && BASE_URL=https://drafthouse.tanmayep.dev pnpm playwright test
```
