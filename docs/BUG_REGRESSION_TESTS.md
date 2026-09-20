# Bug regression tests

Permanent regression suite for [BUG_AUDIT.md](BUG_AUDIT.md). Audit finding **#1 is intentionally not covered** (excluded by request); every other finding (now including **#33**, discovered by the Scylla container tests) plus the duplicate-self indicator bug has a test that asserts the **required** behavior.

**The remaining tests are red on purpose.** Each failure is one unfixed audit finding. A fix must turn its test green **without weakening the assertion**. Do not delete, skip, or loosen a `REG-` test to make a build pass; if a requirement is wrong, change the audit doc and the test together in the same commit.

## Coverage map

| Finding | Test | Location | Status |
|---|---|---|---|
| #2 recovery | `regression_02_rejoin_after_eviction_restores_content` (in-memory) + `regression_02_wal_replay_contract_on_real_scylla` (WAL order/boundary/convergence on real Scylla) | WS suite + `dal/dal/tests/scylla_regression.rs` | **green** |
| #3 offline sync | `REG-03: reconnect handshake uploads edits...` + `regression_03_reconnect_handshake_uploads_offline_edits` | frontend collab suite + WS suite | **green** |
| #4 stale saves | `regression_04_stale_save_cannot_overwrite_newer_text` + `REG-04: the editor receives no plaintext save callback` + `content_patch_is_not_registered` | PG regression + route suite + documents integration suite | **green** |
| #5 initialization | `REG-05` ×2 (duplicate seeding, resurrection/readonly) + `regression_05_server_initializes_content_once` + `regression_05_readonly_update_is_ignored_without_closing` | collab hook + WS suite | **green** |
| #6 revocation | `regression_06_reader_disconnected_when_document_becomes_private`, `regression_06_removed_member_is_disconnected`, `regression_06_downgraded_editor_is_disconnected`, `regression_06_stale_editable_ticket_is_downgraded_on_reconnect`, `regression_06_revoked_member_ticket_is_refused_on_reconnect`, `regression_06_editor_disconnected_after_document_deletion`, `access_events_are_published_only_after_successful_writes` (documents-core unit) | WS suite + documents-core | **green** |
| #7 erasure | `regression_07_deletion_tears_down_live_rooms`, `regression_07_deletion_purges_scylla_data` (Scylla container) | PG suite | **green** |
| #33 Scylla DAL broken | `regression_33_wal_and_snapshot_writes_succeed` | `dal/dal/tests/scylla_regression.rs` (Scylla container) | **green** |
| #8 undo | `REG-08: undo never removes another collaborator's change` | `frontend/src/widgets/editor/__tests__/editorRegression.test.tsx` | red |
| #9 save lock | `REG-09: rapid edits stay in the shared CRDT without plaintext autosaves` | editor suite | **green** |
| #10 final save | `REG-10: navigating away schedules no plaintext final save` | editor suite | **green** |
| #11 async teardown | `REG-11: unmounting during pending ticket setup...` | collab suite | red |
| #12 render lifetime | `REG-12` ×2 (hook + route callback stability) | collab + `frontend/src/routes/__tests__/documentRouteRegression.test.tsx` | red |
| #13 awareness binding | `REG-13: the editor binding tracks the awareness...` | collab suite | red |
| #14 reconnect race | `REG-14: a successful built-in reconnection...` | collab suite | red |
| #15 preview | `REG-15: preview mode keeps receiving collaborators' edits` | editor suite | red |
| #16 title permission | `REG-16` (UI) + `regression_16_editor_member_cannot_rename_document` (server guard) | route + PG suite | red / **green guard** |
| #17 token expiry | `REG-17` ×4 (api refresh+retry, single-flight refresh, refresh-failure, ticket fallback) | `frontend/src/features/documents/__tests__/apiRegression.test.ts` + collab suite | red |
| #18 route race | `REG-18` ×2 (late response, failed load) | route suite | red |
| #19 awareness identity | `regression_19_relay_keeps_identity_and_disconnect_removes_only_owned_clients` | `nanoservices/collab/core/tests/regression.rs` | red |
| #20 name identity | `REG-20: peers with identical display names...` | collab suite | red |
| #21 broadcast lag | `regression_21_lagged_client_is_resynchronized_automatically` | WS suite | red |
| #22 frame handling | `regression_22_update_under_message_limit_is_applied`, `regression_22_fragmented_update_is_aggregated` | WS suite | red |
| #23 upgrade leak | `regression_23_failed_upgrades_do_not_consume_slots` | WS suite | red |
| #24 decoder panic | `regression_24_malformed_length_is_rejected_without_panicking` + `regression_24_malformed_frame_releases_connection_slot` | core + WS suite | red |
| #25 size limit | `regression_25_document_size_limit_is_enforced` | WS suite | red |
| #26 eviction | `regression_26_failed_snapshot_retains_room`, `regression_26_connection_arriving_during_sweep_keeps_room` | core suite | red |
| #27 snapshot timer | `regression_27_due_room_is_snapshotted_by_sweep` | core suite | red |
| #28 latest snapshot | `regression_28_latest_snapshot_is_newest_write_not_highest_slot` | `dal/dal/tests/scylla_regression.rs` (Scylla container) | **green** |
| #29 WAL failure | `regression_29_failed_wal_write_is_not_broadcast` | WS suite | **green** |
| #30 pagination | `regression_30_pagination_returns_all_documents_with_equal_timestamps` | PG suite | red |
| #31 reset validation | `regression_31_reset_rejects_short_password` | PG suite | red |
| #32 token consumption | `regression_32_refresh_token_consumed_exactly_once`, `regression_32_password_reset_token_consumed_exactly_once` | PG suite | red |
| duplicate-self | `REG-SELF` ×2 (avatar of same user, superseded sessions' cursors) | collab suite | red |
| guard: invite concurrency | `guard_invite_link_max_uses_enforced_under_concurrency` — pins the `FOR UPDATE` serialization no existing test exercised | documents suite | **green guard** |
| guard: editor cap | `guard_editor_cap_rejects_101st_connection` — 429 end-to-end through the real handler | WS suite | **green guard** |
| guard: projection failure | `guard_projection_failure_keeps_durable_update_accepted` — WAL durability remains authoritative when the PostgreSQL materialized view is unavailable | WS suite | **green guard** |
| guard: snapshot retention | `guard_snapshot_ring_retains_at_most_five_rows` + core retention test — monotonic generations retain only the latest five | Scylla DAL + core suites | **green guard** |
| guard: presence endpoint | `guard_presence_endpoint_lists_recent_deduplicated_peers_only` — cutoff + per-user dedup (existing test only covered the empty room) | documents suite | **green guard** |

### Known coverage gaps

- **#1 (JWT secret fallback):** excluded by request. When addressed, add a startup test asserting the process refuses the default secret, and a deployment-config check that `JWT_SECRET` is injected.
- **#19:** core ownership and frontend relay seams are covered; a full two-browser end-to-end test would additionally pin the wire behavior.
- Container-backed tests use `testcontainers-modules` 0.15 (`postgres` + `scylladb` features). #28/#33/#7-purge run against a real `scylladb/scylla` container (~30–60s each).

## Running

```sh
# Frontend (regression tests run with the normal suite)
cd frontend && pnpm test

# Rust, no database needed
JWT_SECRET=regression-tests-only \
  cargo test -p collab-core -p collab-networking --test regression

# Postgres + Scylla backed suites (testcontainers; requires a running Docker daemon)
cargo test -p documents-networking --test regression   # PG + Scylla
cargo test -p dal --test scylla_regression            # Scylla only
```

The Postgres-backed tests follow the repo's existing integration-test convention
(`testcontainers-modules` Postgres, one disposable container per test) and run in
the default `cargo test` invocation for the crate — no environment variables or
`--ignored` flags needed. The `JWT_SECRET` value is a test-only secret; it exists
because the production code requires the variable to be set, not because audit #1
is tested.

## Current state (2026-09-20)

- Frontend: **17 tests red**, 148 tests green. #4/#9/#10 are green; the remaining failures are the documented regressions.
- `collab-core`: **5 red** (integration suite); the 36 lib tests are green, including the `access_sync` event-subscriber and purge-retry tests. `collab-networking`: **6 red, 16 green** — #6 revocation is green (single-process); #21–#25 remain red.
- Postgres + Scylla regression suite: **4 red, 6 green** — #4 and #7 (room teardown + Scylla purge) are green; #30/#31/#32 remain red.
- Scylla DAL suite: **4 green** — WAL replay uses monotonic sequences and snapshots use monotonic generations with explicit WAL boundaries.
- `cargo test --workspace --lib` remains fully green; the pre-existing integration tests pass on `testcontainers-modules` 0.15.

## Workflow for fixing

1. Pick a finding; locate its `REG-` test(s) in the map above.
2. Implement the narrow fix (see BUG_AUDIT.md "Fix" notes).
3. Its test(s) turn green; no other test may regress.
4. Update the status column here in the same commit.
