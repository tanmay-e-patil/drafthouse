# Bug audit — collaboration first

Reviewed revision: `75ca88d` plus the existing working tree. No implementation changes made.

**Follow-up:** [Reproduction results](BUG_REPRODUCTIONS.md) cover #2–32 and duplicate-self indicators (29 reproduced, 2 partial). [Permanent regression tests](BUG_REGRESSION_TESTS.md) now assert the required behavior for every finding except #1; they are intentionally red until each bug is fixed. #1 was excluded from the follow-up by request.

This is a source audit with targeted local reproductions, not a guarantee that every bug has been found. No production traffic, credentials, or data were used. Postgres/Scylla integration and multi-browser end-to-end scenarios were not run.

## Verification

- Existing Rust library tests: **124 passed** (`cargo test --workspace --lib`).
- Existing frontend tests: **149 passed** (`pnpm test`).
- Temporary targeted checks reproduced **10 failure behaviors**: empty room recreation, malformed-message panic, awareness identity reassignment/removal, post-unmount connection creation, deleted-content resurrection, duplicate bootstrap content, stale awareness binding after reconnect, offline edits not uploaded, remote-awareness retransmission, and undo deleting a collaborator's edit.
- Temporary harnesses were removed from the repo after execution. Logs/harness copies are under `/tmp/drafthouse-audit-*`, `/tmp/drafthouse-hook-repro.log`, and `/tmp/drafthouse-*-audit-tests.log` on the audit machine.
- **R** below means a targeted reproduction exercised the relevant behavior. **C** means the finding follows from the traced code, but its complete end-to-end scenario was not executed. Passing existing tests does not cover these failures.

## Critical security and data integrity

### 1. Deployment falls back to a publicly known JWT signing secret — P1, C

**Locations:** `nanoservices/auth/core/src/jwt.rs:7–8`; `compose.dokploy.yml:57–64`; `nanoservices/auth/core/src/ws_capability.rs:39–54`.

The backend silently uses `dev-secret-change-in-production` when `JWT_SECRET` is absent. The supplied Dokploy Compose app environment does not pass `JWT_SECRET`, and the Docker entrypoint does not inject it. Unless an external container-level override supplies it, this deployment accepts forged access tokens and forged document write capabilities. Putting a value in the host's `.env` alone does not pass an unreferenced variable into this container.

**Fix:** require a non-default secret at startup, explicitly inject it in deployment configuration, and rotate the key if this configuration has been deployed without an override.

### 2. Saved CRDT state is never restored — P1, R + C

**Locations:** `nanoservices/collab/core/src/room.rs:218–227`; `nanoservices/collab/networking/src/handlers.rs:80`; `dal/dal/src/scylla_txs/collab.rs:40–170`.

Every new room starts with `Doc::new()`. Snapshot readers and WAL replay queries have no production callers. Restarting the process or evicting a room therefore discards its CRDT history. An editor may reconstruct plain text from Postgres, but that is not recovery: it loses unsaved updates and CRDT identities. Existing clients cannot safely reconnect to the rebuilt history. A viewer joining alone cannot hydrate the server document either.

**Fix:** load and validate a snapshot and replay WAL before publishing a room; serialize initialization per document.

### 3. Reconnect never uploads edits made while disconnected — P1, R

**Locations:** `nanoservices/collab/networking/src/handlers.rs:103–110,165–171`.

The comment says SyncStep1, but the server sends a full **SyncStep2**. The client sends its own Step1 and receives another Step2; neither requests the client's missing updates. The real installed `y-websocket` provider reports `synced=true` while the server still lacks offline edits. Later deltas can depend on those missing CRDT structures and remain unapplied.

**Fix:** implement the two-way sync handshake, accounting for read-only clients, and test disconnect/edit/reconnect convergence.

### 4. Plaintext autosaves can overwrite a newer collaborative state — P1, C

**Locations:** `frontend/src/widgets/editor/Editor.tsx:109–130,159–165`; `dal/dal/src/postgres_txs/documents.rs:204–219`.

Every editable client saves the entire text, including after remote changes, via unconditional `UPDATE documents SET content = ...`. There is no revision comparison or serialization across clients. A delayed older request can arrive after a newer save and overwrite it. REST reads, exports, and room reconstruction then use stale text even though the live CRDT had converged.

**Fix:** derive persisted text from the authoritative server CRDT, rather than allowing competing full-text client saves.

**Resolved:** the collaboration server now writes every accepted update to its ordered WAL before applying or broadcasting it, then derives a revision-guarded PostgreSQL projection from the server CRDT. Clients no longer submit whole-document plaintext, and `PATCH /documents/{id}/content` is no longer registered. Content GET remains a read-only view of the materialized projection.

### 5. Client-side initialization duplicates or resurrects content — P1, R

**Location:** `frontend/src/features/collab/useCollabEditor.ts:175–181`.

Each client inserts `initialContent` whenever sync completes and the shared text is empty. Two clients joining an empty room independently insert the same text, and CRDT merging produces two copies. Deleting all text and then resyncing re-inserts the old `initialContent`. Read-only clients also run this insertion; when their update reaches the server, the read-only policy closes the socket.

**Fix:** initialize content exactly once on the server. Empty text must remain a valid document state.

### 6. Revoking access does not revoke an existing connection — P1, C

**Locations:** `nanoservices/collab/networking/src/handlers.rs:47–66,94–100,174`; `nanoservices/documents/core/src/lib.rs:472,496`.

Authorization is captured once in `ConnectionMeta`. Removing a member or downgrading editor to viewer does not close/update that socket. Making a public document private does not disconnect anonymous readers. Deleting a document likewise leaves its live room usable. A removed editor can continue writing, and a removed viewer can continue receiving future confidential changes.

**Fix:** propagate permission/deletion events to active sessions and enforce them immediately. Short-lived handshake tokens do not expire an already-open session.

**Resolved (single-process):** document mutations now publish `DocumentAccessChanged` events after the database write commits; the collaboration subscriber signals the affected room, and sessions selectively close (all on deletion, anonymous on private, only the targeted user on member removal/role change). Reconnects revalidate the ticket against current Postgres policy, so a stale capability can never grant more privileges than the policy now allows (downgraded members reconnect read-only; revoked members are refused). Cross-replica propagation still requires the multi-replica decision noted in the storage findings.

### 7. Account/document deletion leaves collaborative content behind — P1, C

**Locations:** `nanoservices/auth/core/src/me.rs:72–89`; `nanoservices/documents/core/src/lib.rs:202–226`; `dal/dal/src/postgres_txs/documents.rs:64–73`.

Deletion only removes Postgres rows. Scylla snapshots, WAL records, and in-memory rooms are not purged. Snapshots have no automatic expiry. Existing sessions may also keep producing snapshots after deletion. This violates the documented right-to-erasure behavior; it is distinct from the active-access problem above.

**Fix:** coordinate room shutdown with deletion of all document storage, including retryable cleanup on partial failure.

**Resolved (single-process):** deletion (document or account) now tears down the live room — sessions are disconnected, the room is removed from the store and permanently closed to durable writes — and purges every Scylla collaboration row (both the ordered `ops_v2`/`snapshots_v2` tables and the legacy `ops`/`snapshots` tables) with bounded retries. Closing the room under its ordering gate prevents racing sessions from repopulating purged storage. Account deletion captures owned document ids before the Postgres user-row cascade and publishes a deletion event per document. If all purge retries fail the failure is logged loudly; a background reaper for that residual case and cross-replica propagation remain open with the multi-replica ownership decision.

### 8. Undo can delete another collaborator's edit — P1, R

**Location:** `frontend/src/widgets/editor/Editor.tsx:191,201–204`.

The editor installs ordinary CodeMirror `history()` and `historyKeymap` alongside `yCollab`. The installed binding dispatches remote document changes into CodeMirror, where ordinary history records them. A reproduction with the real CodeMirror/Yjs binding received a remote-only edit, ran the configured undo command, and deleted that remote edit from the shared text.

**Fix:** use the collaboration-aware Yjs undo manager/keymap instead of the independent CodeMirror history stack.

## Client lifecycle and saving

### 9. Autosave drops edits made during an in-flight save — P1, C

**Location:** `frontend/src/widgets/editor/Editor.tsx:109–120`.

If request A is slow and the debounce for newer text B fires, `saveLockRef.current` causes B to be discarded. Nothing queues a follow-up save. When A completes, it clears `hasUnsavedChanges`, falsely marking B as saved.

**Fix:** queue the latest pending version and clear dirty state only when that version is persisted.

**Resolved by architecture:** client plaintext autosaves and their save lock were removed. Rapid edits update the shared Yjs document; the server's WAL-first authoritative update path owns persistence.

### 10. Navigation cancels the final pending save — P1, C

**Location:** `frontend/src/widgets/editor/useDebounce.ts:11–19`; `frontend/src/widgets/editor/Editor.tsx:109–130`.

Typing and navigating away within 500ms cancels the timer without flushing the latest content. Postgres remains stale. Because CRDT recovery is absent, the edit can disappear after room eviction/restart even if it reached the live room.

**Fix:** use authoritative CRDT persistence; meanwhile explicitly handle dirty content on navigation rather than silently dropping it.

**Resolved by architecture:** navigation no longer owns a pending plaintext save because that mutation path was removed. Each accepted collaboration update is durable in the server WAL before acknowledgement/broadcast, so component teardown schedules no final REST write.

### 11. Async connection setup can finish after unmount — P1, R

**Location:** `frontend/src/features/collab/useCollabEditor.ts:100–125,224–240`.

`destroyed` is checked only before awaiting the ticket request and dynamic imports. Cleanup during either await sees no provider/view to destroy. The continuation then creates an orphan socket/editor and can mutate global presence/status after another document has opened. The targeted hook test confirmed a provider was created after unmount and never destroyed.

**Fix:** recheck cancellation after async boundaries and clean up every resource created by that specific effect instance.

**Resolved:** every session-setup await rechecks a supersede counter before creating any resource; re-renders supersede their predecessor's in-flight setup, and teardown is deferred by one tick so a following effect (the re-render handoff) cancels it while a real unmount destroys everything.

### 12. Ordinary parent renders destroy the collaboration session — P2, C

**Locations:** `frontend/src/routes/documents.$documentId.tsx:193–202,369`; `frontend/src/widgets/editor/Editor.tsx:215–227`; `frontend/src/features/collab/useCollabEditor.ts:240`.

`handleRemoteTitleUpdate` is a new function on every parent render. It changes `collabOptions`, which tears down the entire effect and Y.Doc. Typing a title, receiving a remote title, or toggling parent UI therefore unnecessarily reconnects and resets editor state/selection. Unsynced local state is particularly vulnerable.

**Fix:** stabilize callbacks or use callback refs; do not tie the document/provider lifetime to ordinary presentation options.

**Resolved:** the collaboration session is keyed by document and survives option/callback identity changes (callbacks are read through the session's latest options), and the route's title callback is memoized. Presentation changes that intentionally end the session (preview/leave) still tear it down.

### 13. Reconnect creates a new awareness instance but keeps the old editor binding — P2, R

**Location:** `frontend/src/features/collab/useCollabEditor.ts:120–127,185–218`.

Reconnect replaces the provider and its awareness, but `if (!view)` skips rebuilding the existing editor. `yCollab` and the activity listener still use the first awareness object; the socket/store use the second. Cursor and activity synchronization breaks after reconnect. The targeted hook test confirmed the identities differ.

**Fix:** preserve a single awareness object or explicitly reconfigure the editor binding when replacing the provider.

**Resolved:** replacing a provider reconfigures the existing editor in place (`StateEffect.reconfigure` with the new `yCollab` binding), so the yCollab facet always tracks the current provider's awareness without discarding document or selection.

### 14. Two reconnect mechanisms race, and successful reconnects leave timers armed — P2, C

**Location:** `frontend/src/features/collab/useCollabEditor.ts:165–172,211–221`.

`y-websocket` already automatically reconnects. The hook schedules a second reconnect that destroys the provider, without cancelling that timer on `connected` or deduplicating timers. A fast built-in recovery is torn down again by the old timer. Destroying a connected provider itself emits `disconnected`, which can schedule yet another replacement.

**Fix:** use one reconnect owner; cancel/deduplicate retries and refresh ticket parameters through that lifecycle.

**Resolved:** the hook's reconnect timer is the single retry owner: pending timers are cleared when the provider reports connected again (so a successful built-in recovery is never torn down), timers never stack, and each retry reconnects with a freshly issued ticket.

### 15. Preview mode stops receiving collaborators' edits — P2, C

**Location:** `frontend/src/widgets/editor/Editor.tsx:215–227,253–265`.

For an editable user, selecting Preview sets collaboration options to null and removes the editor container, destroying the connection. The preview renders the last local string indefinitely while other users edit. Read-only users follow a different path and retain a hidden collaboration editor.

**Fix:** keep the shared document/provider alive independently of the edit/preview presentation.

**Resolved:** edit and preview now share one continuously mounted collaboration container and stable hook options. Preview hides the CodeMirror presentation without destroying its Yjs session, so remote updates continue updating the rendered Markdown and switching back preserves the same editor state.

### 16. Editor members are offered title editing that always fails — P2, C

**Locations:** `frontend/src/routes/documents.$documentId.tsx:162–164,276`; `nanoservices/documents/core/src/lib.rs:182–187`.

The UI enables title changes for editor members and only excludes viewers. The backend permits title updates only for the owner, so every editor-member rename fails with 403 and is reverted.

**Fix:** align the title control with the intended owner/editor permission policy.

**Resolved:** title editing now follows the backend's owner-only policy. The title input is disabled for editor and viewer members, the blur handler independently refuses non-owner updates, and owners retain the existing rename flow.

### 17. Access tokens expire during normal long editing sessions without refresh — P1, C

**Locations:** `frontend/src/features/auth/store.ts:41–47`; `frontend/src/features/documents/api.ts:45–51`; `frontend/src/features/collab/useCollabEditor.ts:106–113`; `nanoservices/auth/core/src/jwt.rs:11–15`.

Token refresh occurs only on page hydration; API calls neither refresh proactively nor retry 401 after refresh. After the default 15 minutes, autosaves and management operations fail. A reconnect's ticket request also fails and silently falls back to anonymous access: private documents cannot reconnect, and public editors reconnect as read-only.

**Fix:** centralize single-flight token refresh/retry and distinguish authorization failure from a legitimate anonymous-public connection.

**Resolved:** all document/collaboration API calls now go through one authenticated fetch helper: a 401 triggers the shared single-flight refresh and retries the original request exactly once with the new token, surfacing any subsequent error instead of looping. A failed refresh clears only the access token. The collaboration hook no longer silently downgrades an authenticated session to an anonymous socket when its ticket request fails — it reports disconnected and retries with backoff, so private documents cannot appear as read-only/public after an expiry.

### 18. Overlapping document loads can bind one document's text to another document ID — P1, C

**Location:** `frontend/src/routes/documents.$documentId.tsx:126–154,363–369`.

`fetchDocument` has no abort/generation guard, and the route component is reused between document IDs. If A's slow request completes after navigation to B, it overwrites state with A's document/content while the editor's `docId` and save callback still use B. A transient failure loading B also leaves the previous document state intact. Subsequent saves or empty-room initialization can target B with A's text.

**Fix:** cancel/ignore stale loads, reset document state on navigation, and require state identity to match the current route before mounting an editor.

**Resolved:** each document load now owns an effect-scoped active flag, so cleanup prevents obsolete success, error, and completion handlers from mutating route state. Navigation resets all document-bound editor state before loading, and the route refuses to mount an editor when the loaded document ID differs from the current route ID.

## Server protocol, presence, and persistence

### 19. Awareness relays are attributed to the wrong authenticated user — P2, R

**Locations:** `nanoservices/collab/networking/src/handlers.rs:213–216,258–275`; `nanoservices/collab/core/src/room.rs:174–209`.

The real `y-websocket` provider retransmits awareness changes it receives for other peers. The server assumes every client ID in a socket's awareness payload belongs to that socket's authenticated user, overwriting `user_id` and tracking those IDs as owned by the relaying connection. When that connection leaves, it removes other users' presence too. This occurs with normal clients, not only malicious payloads; payload clocks are also ignored in the server presence map.

**Fix:** maintain protocol-correct awareness clocks and stable ownership/identity, rather than deriving all relayed client identities from the latest sender.

### 20. Distinct users disappear from the avatar list because names are treated as identities — P2, C

**Location:** `frontend/src/features/collab/useCollabEditor.ts:25–27,76–97`.

Peers are deduplicated by the email local-part display name. `alice@company-a.com` and `alice@company-b.com` collapse into one entry, and all anonymous readers collapse into `Anonymous`. This can also hide another peer when excluding the local client.

**Fix:** deduplicate authenticated users by user ID and anonymous sessions by client ID.

**Resolved (test-scoped):** peers are no longer collapsed by display name — distinct users with identical names remain distinct — and the local user's own sessions (including superseded ones) are excluded from the avatar list by name and local client ID. Identity is still name-based because awareness payloads carry no user id; full user-ID identity lands with the #19 awareness-identity work.

### 21. Broadcast lag silently loses required CRDT updates — P1, C

**Locations:** `nanoservices/collab/core/src/room.rs:26`; `nanoservices/collab/networking/src/handlers.rs:141–143`; `frontend/src/features/collab/useCollabEditor.ts:124`.

The bounded channel stores 256 messages, but the select branch only matches `Ok(bytes)`. `RecvError::Lagged` is discarded without resynchronizing or closing the connection. A slow client can permanently miss operations while remaining connected; periodic resync is explicitly disabled. WAL writes awaited inside a session's receive loop increase the chance of lag. Follow-up socket tests also show that a lag error stalls the broadcast arm until another incoming event wakes the current `select!`; even after delivery resumes, only an explicit full resync repairs the missing predecessor.

**Fix:** treat lag as a required resync/reconnect, never as a ignorable transport event.

### 22. Valid 64–100 KiB updates are silently dropped; fragmented binary messages are ignored — P1, C

**Location:** `nanoservices/collab/networking/src/handlers.rs:87,119–137`.

The advertised application limit is 100 KiB, but `actix-ws` 0.3.1 defaults to a 64 KiB frame limit and the handler never changes it. An update between those sizes produces a stream error swallowed by `_ => {}`. Continuation frames are also ignored rather than aggregated. A sufficiently large paste/import can appear locally and in plaintext saves but never reach collaborators.

**Fix:** configure consistent frame/message limits, aggregate continuations, and close/report protocol errors explicitly. Design a transfer path for large legitimate sync updates.

### 23. Failed WebSocket upgrades permanently consume room connection slots — P1, C

**Location:** `nanoservices/collab/networking/src/handlers.rs:80–87`.

`add_connection()` runs before `actix_ws::handle(...) ?`. An invalid upgrade returns early without decrementing. Repeating an ordinary non-upgrade request to an authorized/public collaboration URL can consume all 100 slots; the room never becomes empty for eviction.

**Fix:** acquire the slot only after successful upgrade validation, or release it with an ownership guard on every exit path.

### 24. Malformed protocol lengths panic outside the safety boundary — P1, R

**Location:** `nanoservices/collab/core/src/sync_protocol.rs:64–71`.

`read_bytes` adds an untrusted varint length to the position without checked arithmetic. A tiny frame encoding `usize::MAX` as the payload length panics in debug builds; unchecked wrapping also leads to invalid slice bounds in release. `decode_message` runs outside `apply_update_safe`'s panic guard. A session panic skips normal connection/presence cleanup and can exhaust room slots.

**Fix:** use checked length arithmetic and reject malformed frames before slicing; use cleanup guards independent of normal loop completion.

### 25. The document size limit is never enforced — P1, C

**Locations:** `nanoservices/collab/core/src/room.rs:17`; `nanoservices/collab/networking/src/handlers.rs:174–206`.

`MAX_DOC_BYTES` is declared but has no enforcing caller. Many individually small updates can grow a room without bound, increasing memory usage and full-state/snapshot cost despite the nominal 1 MiB limit.

**Fix:** enforce an explicit document resource policy at the authoritative update boundary and communicate rejection to the client.

### 26. Eviction can remove a newly active room and discards it before snapshot success — P1, C

**Location:** `nanoservices/collab/core/src/snapshot.rs:45–55`.

The sweep collects idle IDs and later unconditionally removes them, without atomically rechecking activity. A connection joining between those steps keeps the old Arc while the next join creates a separate room for the same document. The room is removed before persistence is awaited, and snapshot failure is ignored, so failed persistence also loses the live recovery source.

**Fix:** coordinate room acquisition and eviction, revalidate idleness under that coordination, and do not discard the room until persistence succeeds.

### 27. The advertised 30-second snapshot timer does not exist — P2, C

**Locations:** `nanoservices/collab/core/src/room.rs:107–113`; `nanoservices/collab/networking/src/handlers.rs:205–208`; `ingress/src/main.rs:73–83`.

Elapsed time is checked only when a new document update arrives. A user can make one edit and leave the document open indefinitely without a snapshot. Awareness traffic does not trigger it, and eviction requires no connections. After seven days, the corresponding WAL can expire without any snapshot covering that edit.

**Fix:** run a real periodic dirty-room snapshot task in addition to the operation threshold.

### 28. “Latest snapshot” selects the highest ring slot, not the newest snapshot — P2, C

**Locations:** `dal/dal/src/scylla_txs/collab.rs:112`; `nanoservices/collab/core/src/room.rs:117–125`.

Versions wrap from 5 back to 1, but the reader orders by version descending. After writing slot 1 again, slot 5 is returned as latest. This is currently a latent recovery defect because recovery itself is missing; wiring the existing reader into recovery would select stale state.

**Fix:** select by a monotonic generation/timestamp rather than the reusable slot number.

### 29. WAL failure is treated as successful collaboration — P1, C
**Location:** `nanoservices/collab/networking/src/handlers.rs:185–207`.

The server applies an update, ignores `write_op` errors, and broadcasts the change anyway. There is no retry or error signal. During Scylla outages clients continue believing edits are accepted, but a crash before a successful snapshot loses them. This is an unbounded outage window, not just a documented short async buffer window.

**Fix:** define and implement durable acknowledgement/retry semantics; surface persistence failure instead of silently discarding it.

## Additional repo findings

### 33. The Scylla DAL fails against real ScyllaDB: WAL and snapshot persistence has silently never worked — P1, reproduced (testcontainers)

**Locations:** `dal/dal/src/scylla_txs/collab.rs` (`write_op`, `write_snapshot`, `read_latest_snapshot`, `read_ops_since`, `read_all_snapshots`).

Discovered while adding the Scylla testcontainer regression tests: the DAL binds raw `i64` milliseconds to CQL `timestamp` columns (`ops.created_at`, `snapshots.taken_at`) and deserializes them back as `i64`. The pinned `scylla` 0.15 driver type-checks `i64` against `BigInt` only and requires `CqlTimestamp`/chrono types for `Timestamp` — so every write **and** read of those columns fails against a real ScyllaDB. The failures are swallowed upstream (`let _ = dal.write_op(...)`, `warn`-only `persist_snapshot`, ignored eviction result), so nothing surfaced this: the WAL and snapshots have plausibly never persisted in any real deployment. This makes #2 (no recovery) and #29 (silent WAL failure) strictly worse — there is no durable state to recover from at all.

**Fix:** bind and deserialize `CqlTimestamp` (or chrono types) in the Scylla DAL, and stop swallowing persistence errors (see #29). Regression tests: `dal/dal/tests/scylla_regression.rs` (`regression_33_...`, `regression_28_...`).

### 30. Pagination skips documents sharing the cursor timestamp — P2, C

**Location:** `dal/dal/src/postgres_txs/documents.rs:95–96`.

The cursor filter uses only `updated_at < cursor.updated_at`, and ordering has no ID tie-breaker. Documents with equal timestamps at the page boundary are omitted permanently from subsequent pages. Concurrent updates can also move the cursor's timestamp because the cursor stores only the ID.

**Fix:** use a stable compound `(updated_at, id)` cursor and matching ordering/filtering.

### 31. Password reset bypasses minimum password validation — P2, C

**Location:** `nanoservices/auth/core/src/password_reset.rs:90–94`.

Registration and change-password reject passwords under eight bytes, but reset hashes and stores any supplied string, including an empty password. Frontend validation does not protect the API.

**Fix:** share the same server-side password validation across registration, change, and reset.

### 32. One-time auth token consumption is not atomic — P2, C

**Locations:** `nanoservices/auth/core/src/login.rs:96–134`; `nanoservices/auth/core/src/password_reset.rs:63–96`.

Refresh reads a token, deletes it, then creates a replacement through separate calls; concurrent requests can both read and successfully reuse the same token. Reset similarly checks `used_at`, updates the password, and marks the token used separately, so simultaneous requests can both reset the password. Partial database failures can leave credentials changed without token consumption/session revocation being completed.

**Fix:** consume tokens conditionally and mutate associated auth state within a transaction, with a single successful consumer.

## Suggested remediation order

1. Verify deployment secret injection immediately; fix the default-secret configuration.
2. Fix authoritative CRDT recovery, two-way reconnect sync, initialization, and plaintext persistence ownership.
3. Add live permission revocation and complete deletion cleanup.
4. Fix undo, autosave, route/effect lifecycle, and token refresh.
5. Harden protocol limits/cleanup, broadcast lag, eviction, snapshots, and awareness.
6. Add real two-client tests covering concurrent edits, disconnect/reconnect, server restart, empty documents, role changes, lag, large payloads, and undo. Keep mock-only component tests, but do not treat them as collaboration correctness tests.
