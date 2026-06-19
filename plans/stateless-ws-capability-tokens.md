# Plan: Stateless WebSocket Capability Tokens

## Goal

Make `/collab/{doc_id}` WebSocket admission fast by removing Postgres ticket lookup/delete from the hot path.

Current hot path:

```text
POST /documents/{id}/ws-ticket
  SELECT document/member
  INSERT ws_tickets

GET /collab/{id}?ticket=...
  SELECT ws_ticket
  DELETE ws_ticket
  SELECT document
  maybe SELECT document_member
  upgrade
```

Target hot path:

```text
POST /documents/{id}/ws-ticket
  SELECT document/member
  sign short-lived capability token

GET /collab/{id}?ticket=...
  verify token locally
  check doc_id/path match
  upgrade
```

## Success criteria

- No Postgres calls for valid ticket verification in `collab` WS handler.
- Existing permission behavior preserved at ticket issue time.
- Viewer/editor readonly behavior preserved.
- Expired/wrong-doc/malformed tickets rejected.
- Load test WS connect p95 improves from current ~3s toward `<1s` at 10 VUs.

## Phase 1: Measure before changing

Add temporary or permanent tracing spans around:

- `documents.issue_ws_ticket.resolve_access`
- `documents.issue_ws_ticket.sign_or_store_ticket`
- `collab.ticket.verify`
- `collab.ticket.delete`
- `collab.access.resolve_readonly`
- `collab.room.get_or_create`
- `collab.ws.upgrade`
- `collab.initial_sync`

Verify with one 10-VU WS run.

Deliverable:
- Baseline note in `plans/load-test-results/`.

## Phase 2: Add capability token type

Create a small token module, probably in `documents_core` or shared auth/core if reuse is useful.

Payload:

```rust
struct WsCapabilityClaims {
    sub: Uuid,
    doc_id: Uuid,
    readonly: bool,
    exp: usize,
    iat: usize,
}
```

Use existing JWT secret/config if available. Keep TTL at current 30 seconds.

Tests:
- signs and verifies valid token
- rejects expired token
- rejects malformed token
- rejects wrong secret/token

## Phase 3: Stop storing WS tickets

Change `documents_core::issue_ws_ticket`:

- keep `resolve_document_access(...)`
- compute `readonly` from resolved role
- return signed token as `WsTicketResponse { ticket }`
- remove `CreateWsTicket` bound from this function

Do not delete DB table/migrations yet. Leave unused table for rollback.

Tests:
- owner token has `readonly=false`
- editor token has `readonly=false`
- viewer token has `readonly=true`
- public/unauthorized behavior unchanged

## Phase 4: Make collab verify locally

Change `nanoservices/collab/networking/src/handlers.rs`:

- replace `get_ws_ticket_by_hash`
- remove `delete_ws_ticket`
- verify signed token
- reject if `claims.doc_id != path doc_id`
- use `claims.sub` and `claims.readonly`

Public-doc no-ticket path can stay as-is for now; it still does one document lookup. That is okay because authenticated editor WS is the measured issue.

Tests:
- valid token upgrades
- wrong doc token rejected
- expired token rejected
- readonly token cannot edit

## Phase 5: Clean DAL dependencies

Remove now-unused trait bounds/imports from collab handler:

- `GetWsTicketByHash`
- `DeleteWsTicket`
- ticket hashing helper if no longer used

Keep DAL methods and migration for now. Delete later only after deploy confidence.

## Phase 6: Load-test again

Run:

```bash
WS_SMOKE_VUS=10 WS_SMOKE_DURATION=5m ./load-tests/run.sh
```

Compare:

- `ws_connecting p95`
- HTTP p95 for ticket issue
- WS upgrade success
- app/DB CPU

Then run 25 VUs if 10 passes:

```bash
WS_SMOKE_VUS=25 WS_SMOKE_DURATION=5m ./load-tests/run.sh
```

## Phase 7: Permission revocation follow-up

Not required for the first fix.

Later, when member removal/role changes should kick live users:

- publish `DocumentAccessChanged { doc_id, user_id }`
- collab room closes affected user's active connections
- token TTL remains short for stale reconnects

## Rollback

Because the DB-backed ticket table remains, rollback is just reverting code to old ticket issue/verify flow.

## Non-goals

- No Redis nonce cache.
- No single-use replay protection unless abuse appears.
- No deletion of `ws_tickets` migration/table in this change.
- No full Yjs payload load-test changes in this plan.
