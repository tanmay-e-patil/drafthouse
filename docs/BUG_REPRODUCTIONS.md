# Audit reproduction results

> **Superseded:** the throwaway harnesses described below were replaced by the permanent regression suites in [BUG_REGRESSION_TESTS.md](BUG_REGRESSION_TESTS.md) (`nanoservices/*/tests/regression.rs` and `frontend/src/**/__tests__/*Regression.test.tsx`). This report is kept as the historical reproduction evidence.

Revision: `75ca88d`. Run: 2026-09-20 UTC. Scope: audit **#2–32 plus duplicate-self indicators**. **#1 was deliberately excluded.**

## Summary

- **29 findings reproduced at the tested component/handler/database boundary.**
- **2 partially reproduced:** #7's surviving rooms were confirmed, but Scylla erasure was not exercised; #28's ring-slot ordering mismatch was confirmed, but the actual CQL reader was not executed.
- **Duplicate-self cursors reproduced through the real hook lifecycle**, not just manually fabricated peer state. An extra self avatar was also reproduced. Multiple same-name avatars in a single avatar strip were **not** reproduced; name deduplication limits that case to one extra avatar.
- **49 diagnostic checks passed:** 22 frontend, 8 collaboration-core, 12 collaboration-networking, 7 Postgres-backed.
- Existing suites remain green: **149 frontend tests, 124 Rust library tests**.
- The 22 frontend diagnostic checks passed three additional consecutive repeat runs. Targeted Rust Clippy completed without warnings; the new Rust test files were formatted with rustfmt.
- No implementation code, dependencies, migrations, or production data changed. Only opt-in diagnostic tests and documentation were added. The temporary Postgres cluster was stopped and removed afterward; run logs remain under `/tmp/drafthouse-repro-logs/` on the audit machine.

**Important:** these are characterization tests. A passing diagnostic check means the asserted faulty behavior occurred. They are not evidence that the product is correct. Convert the relevant assertions into healthy-behavior regression tests when fixing each issue.

## What was real, and what was substituted

- **Frontend hook tests:** real React, `useCollabEditor`, Yjs, `y-websocket`, CodeMirror, `y-codemirror.next`, stores, and avatar component. Browser WebSocket transport and ticket API are controlled; BroadcastChannel is disabled to isolate socket behavior. Real awareness frames are relayed explicitly.
- **Editor tests:** actual Editor UI, its configured extensions, undo, debounce, and save lock. Only the connection hook is substituted with a small local Yjs/CodeMirror binding to isolate editor behavior.
- **Route tests:** actual document route component, controlled router parameters and API responses, stubbed child UI. These test the actual route state/save callbacks, not a reimplementation.
- **Rust networking tests:** unchanged production `handlers.rs` is compiled via `include!`; only DAL adapters are substituted with in-memory implementations. Tests run an actual local Actix HTTP/WebSocket server and send actual masked WebSocket frames over TCP. The mock storage can deliberately fail or pause WAL writes.
- **Postgres tests:** actual migrations, DAL SQL, and core functions against a newly initialized, isolated PostgreSQL 18 database. Concurrency tests insert barriers **after real token reads**, leaving all mutation SQL unchanged.
- **Scylla:** unavailable. Docker CLI reported the daemon was not running, and no Scylla listener was available on localhost:9042. No Docker stack or existing database was modified. CQL-specific checks remain limited as noted below.
- No full browser + real Postgres + real Scylla end-to-end run was performed. Different variants of a finding are distinguished below rather than assuming one reproduction proves every variant.

## Per-finding results

Test file abbreviations:

- **Hook:** `frontend/audit/collaboration.repro.tsx`
- **Editor:** `frontend/audit/editor.repro.tsx`
- **Route:** `frontend/audit/route.repro.tsx`
- **Core:** `nanoservices/collab/core/tests/audit_repro.rs`
- **WS:** `nanoservices/collab/networking/tests/audit_repro.rs`
- **PG:** `nanoservices/documents/networking/tests/audit_repro.rs`

Frontend test names contain `#N`; Rust names contain `audit_NN`.

| Audit | Result | Test(s) and observed evidence |
|---|---|---|
| #2 Recovery | Reproduced | Core + WS: persist an edit, close the socket, perform actual eviction/snapshot, reconnect. Snapshot/WAL remain in the storage adapter, but the new room is empty. Real Scylla restart not exercised. |
| #3 Offline sync | Reproduced | Hook: real provider already has offline text, receives the server's current Step2/Step1-response sequence, reports synced, and sends **zero uploads**. Server replica remains empty. Transport sequence is controlled to match the handler. |
| #4 Stale saves | Reproduced | PG: release an older request after a newer save finishes. Both core calls succeed; real Postgres ends with `old text`. |
| #5 Initialization | Reproduced | Hook: two clients merge into `originaloriginal`; clearing and resyncing resurrects `original`; read-only hook also inserts content. WS: read-only insertion receives policy close **1008**. |
| #6 Revocation | Reproduced, scoped | WS: an already-connected anonymous reader receives an edit after the backing document becomes private; an existing editor writes after backing document deletion. Member removal/downgrade was not separately exercised through the management endpoint. |
| #7 Erasure | **Partial** | PG: actual document/account deletion removes Postgres data but leaves active rooms/connections in the DocStore. Actual Scylla WAL/snapshot purge and retry behavior remain untested. |
| #8 Undo | Reproduced | Editor: remote-only append becomes visible; the configured CodeMirror undo removes that append from the shared Yjs text. |
| #9 Save lock | Reproduced | Editor: save A remains pending while edit B's debounce fires. Only A is submitted; B remains visible but is never sent after A completes. |
| #10 Final save | Reproduced | Editor: edit, unmount before debounce, advance time. Save callback is never called. Later persistence loss is not separately simulated here. |
| #11 Async teardown | Reproduced | Hook: unmount with ticket unresolved, then resolve it. A real provider remains connecting and a real CodeMirror editor is constructed after cleanup. |
| #12 Render lifetime | Reproduced | Route: typing title changes `onTitleUpdate` identity. Hook: callback identity change destroys the original provider and creates a new Y.Doc/provider. |
| #13 Awareness binding | Reproduced | Hook: reconnect replaces provider awareness; actual CodeMirror `ySyncFacet` still references the first provider's awareness. |
| #14 Reconnect race | Reproduced | Hook: library reconnect succeeds at 150ms, then stale hook timer destroys it around 1s. That destruction schedules another replacement, producing three providers. |
| #15 Preview | Reproduced | Editor: Preview passes null to the collaboration hook. A later Yjs change does not update the preview text. |
| #16 Title permission | Reproduced | Route: editor member has enabled title field and submits PATCH. PG: that editor can edit the body, but the title core call returns Forbidden. |
| #17 Expiry handling | Reproduced, injected expiry | Editor/API: a 401 save makes one request with no refresh/retry. Hook: a rejected ticket request creates an unauthenticated socket. Did not wait 15 wall-clock minutes; expiry responses were injected. |
| #18 Route race | Reproduced | Route: resolve B first, then delayed A. Editor receives `docId=B`, `initialContent=A text`; pressing save submits **A text to B**. Transient B-load failure reproduces the same stale-state pairing. |
| #19 Awareness identity | Reproduced across boundaries | Hook: real provider retransmits remote awareness. Core: relaying via another connection changes the peer's user ID; disconnecting that relay removes the original peer. Not a combined two-browser/real-DB test. |
| #20 Name identity | Reproduced | Hook: awareness retains three clients, but two distinct `bob` clients collapse to one stored peer. |
| #21 Broadcast lag | Reproduced | Core + WS: stall a socket on WAL, enqueue >256 broadcasts, release it. Broadcast delivery first stalls until an inbound ping; subsequent deltas lack predecessor, leaving replica empty while room is `AB`. Only an explicitly requested full sync repairs it. |
| #22 Frame handling | Reproduced | WS: ~70 KiB valid update produces no broadcast and no text change, but the same socket accepts a later small update. A valid two-frame fragmented binary update is also ignored. |
| #23 Upgrade leak | Reproduced | WS: 100 ordinary non-upgrade GETs each return 400 but increase room connections. The next valid upgrade returns **429**. |
| #24 Decoder panic | Reproduced | Core + WS: a 12-byte malicious-length message panics the session and leaves connection count at 1 after the socket ends. Debug build tested; release behavior not executed. |
| #25 Size limit | Reproduced | WS: 22 accepted 50,000-character updates produce **1,100,000 characters**, exceeding the 1 MiB policy. |
| #26 Eviction | Reproduced | Core: injected snapshot failure still discards room. Separately, a snapshot callback activates another candidate after idle IDs were collected; sweep removes that active room and next lookup creates a distinct Arc. |
| #27 Snapshot timer | Reproduced, scoped | WS: send one edit, hold connection for **31 real seconds**, observe `should_snapshot=true` with zero snapshots; invoking the same sweep used by ingress does not save it. The entire ingress binary and seven-day WAL expiry were not exercised. |
| #28 Latest snapshot | **Partial** | Core: six snapshots yield slots 1,2,3,4,5,1; highest slot contains older data than last write. Actual CQL `ORDER BY version DESC LIMIT 1` was not run because Scylla is unavailable. |
| #29 WAL failure | Reproduced | WS: storage returns an injected WAL error; update still appears in room and is broadcast, with zero persisted ops. |
| #30 Pagination | Reproduced | PG: three documents share exact timestamp; first page returns one and `has_more=true`, second page is empty despite two unseen rows. |
| #31 Reset validation | Reproduced | PG: reset accepts empty password; real password verification succeeds against the stored hash using `""`. |
| #32 Token consumption | Reproduced | PG: synchronize two reads of one refresh token, then both refresh calls succeed and create two replacement rows. Two concurrent reset calls using one reset token also both succeed. Partial database-failure variants were not exercised. |

## Duplicate-self indicators: confirmed chain

`SELF: orphaned hook sessions produce two live cursor labels for the same user` tests this chain:

1. Keep ticket requests pending.
2. Render the same hook/container three times with new title callbacks, matching the route's callback identity behavior.
3. Resolve all three pending ticket requests after the first two effects were cleaned up.
4. Observe **three real CodeMirror editors and three live providers** in the same container. The old two are orphaned, not stopped.
5. Give the sessions the same shared document; focus/edit selections in the orphaned editors.
6. Relay their real emitted awareness frames to the current provider.
7. Observe **two remote cursor labels both reading `alice`** in the current editor.

This directly connects **#11 (async setup after cleanup)** and **#12 (unstable effect identity)** to duplicate-self cursor indicators. Cursor rendering distinguishes client IDs, not authenticated users. The separate reconnect bugs can worsen lifecycle churn, but this reproduction does **not** require them.

A separate real `AvatarStrip` test confirms that a more recently active session with the same display name can be selected by name deduplication and rendered as a collaborator because only the current client ID is excluded. It produces **one extra self avatar**, not multiple identical avatars in the same strip. If the reported symptom specifically means two or more same-name avatars in one strip, that exact variant still needs a screenshot/browser reproduction.

## Audit refinements

- #21 is worse than just dropped deltas: a lag error disables the broadcast branch of the current `select!` until another incoming event wakes the loop. The real socket test reproduces that stall, then demonstrates persistent divergence after delivery resumes.
- #24's live-socket test confirms the leaked connection count after panic; the earlier audit only inferred cleanup loss.
- #28 remains a **latent** recovery problem, since the production reader is not called today. The slot-order test alone is not a Scylla query reproduction.
- The original audit's temporary `/tmp` harnesses were no longer present at the start of this follow-up. This run adds persistent, rerunnable harnesses to the repository instead.

## Running the diagnostic suites

### Frontend — no services required

```sh
cd frontend
pnpm exec vitest run --config audit/vitest.config.ts
```

Use a name filter for a specific finding, e.g. `-t 'SELF:'` or `-t '#18'`.

### Rust collaboration — no database required

```sh
JWT_SECRET=audit-only-not-for-production \
  cargo test -p collab-core -p collab-networking \
  --test audit_repro -- --ignored
```

The networking suite binds ephemeral loopback ports. One test deliberately waits 31 seconds. A panic message from the malformed-frame test is expected; the test asserts the failure and cleanup defect. The test-only signing secret is explicit; audit #1 is not tested.

### Real Postgres checks — disposable database only

On the audit machine, PostgreSQL 18 was available locally. Example setup, using a **new temporary cluster**, not an existing database:

```sh
PG_BIN=/opt/homebrew/opt/postgresql@18/bin
PG_TMP=$(mktemp -d /tmp/drafthouse-audit-pg.XXXXXX)
"$PG_BIN/initdb" -D "$PG_TMP/data" -A trust -U audit
"$PG_BIN/pg_ctl" -D "$PG_TMP/data" -l "$PG_TMP/server.log" \
  -o '-h 127.0.0.1 -p 55439' start
"$PG_BIN/createdb" -h 127.0.0.1 -p 55439 -U audit drafthouse_audit

AUDIT_DATABASE_URL=postgres://audit@127.0.0.1:55439/drafthouse_audit \
JWT_SECRET=audit-only-not-for-production \
  cargo test -p documents-networking --test audit_repro -- --ignored

"$PG_BIN/pg_ctl" -D "$PG_TMP/data" -m fast stop
# The temporary directory can now be removed.
```

Tests create uniquely named test users/documents and run the repository migrations. Never point `AUDIT_DATABASE_URL` at production or a valuable development database.

### Baseline checks

```sh
cargo test --workspace --lib
(cd frontend && pnpm test)
```

Frontend `tsc --noEmit` also ran. The audit files have no type errors; the command remains blocked by two existing errors in `frontend/src/routes/-indexLanding.tsx` (unused `Check` import and an icon/component union rendered as a ReactNode). Those unrelated files were not changed.

## Remaining validation

1. Run actual Scylla snapshot ordering and erasure checks in a disposable Scylla environment.
2. Exercise membership downgrade/removal through management APIs while full-stack sockets are open.
3. Confirm the user's exact indicator variant in a real browser; automated reproduction currently proves duplicate self cursors and one extra self avatar.
4. Before fixing each bug, convert its characterization assertion into the expected healthy behavior, verify it fails, implement the narrow fix, then rerun relevant suites.
