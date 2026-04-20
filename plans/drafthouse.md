# Plan: Drafthouse — Collaborative Markdown Editor

> Source PRD: GitHub Issue #1

## Architectural Decisions

Durable decisions that apply across all phases:

- **Routes:** `/auth/*`, `/documents/*`, `/invites/:token/accept`, `/collab/:doc_id` (WS), `/documents/:id/ws-ticket`
- **Schema (Postgres):** `users`, `refresh_tokens`, `password_reset_tokens`, `email_verification_tokens`, `documents`, `document_members` (`member_role` enum: `editor`|`viewer`), `invite_links`, `ws_tickets`
- **Schema (ScyllaDB):** `ops` (WAL, 7-day TTL, TIMEUUID clustering), `snapshots` (ring buffer, version 1–5 per doc)
- **Key models (dal/kernel):** `User`, `Document`, `DocMember`, `InviteLink`, `WsTicket`, `MemberRole`
- **Auth:** Argon2id passwords, 15-min JWT (Zustand memory), 30-day refresh token (httpOnly SameSite=Strict cookie), email verification required before access
- **WS auth:** one-time ticket (30s), burned on connect, stored hashed in `ws_tickets`
- **CRDT:** Yrs (y-crdt), y-websocket binary protocol, one `DocRoom` per active doc in process memory
- **Nanoservice layers:** networking (Actix handlers) → core (business logic) → dal (DB queries). Three services: `auth`, `documents`, `collab`
- **API style:** REST + OpenAPI (`utoipa` → `openapi-ts`), RFC 7807 errors, cursor pagination on doc list
- **Frontend:** TanStack Start + FSD, CodeMirror 6 + y-codemirror.next, TanStack Query + Zustand, shadcn/ui + Tailwind

---

## Phase 1: Foundation + Auth

**User stories:** 1–12

### What to build

End-to-end auth slice: a user can register, verify their email, log in, silently refresh their session, reset their password, and log out — all backed by real Postgres and a real frontend.

Repo scaffold ships in this phase: Cargo workspace with all crate stubs, FSD frontend skeleton, Docker Compose (Postgres + app), CI pipeline (fmt/clippy/test/tsc/vitest), Makefile targets, Infisical config skeleton, Postgres migrations for all auth tables.

Auth API: `POST /auth/register`, `POST /auth/login`, `POST /auth/refresh`, `POST /auth/logout`, `POST /auth/logout-all`, `GET /auth/me`, `POST /auth/forgot-password`, `POST /auth/reset-password`, `POST /auth/verify-email`, `POST /auth/resend-verification`.

Frontend: `/login`, `/register`, `/verify-email`, `/forgot-password`, `/reset-password` routes. JWT stored in Zustand. TanStack Query global 401 → refresh → retry. Refresh token via httpOnly cookie.

Error/empty states: form validation errors inline, toast for server errors (wrong password, expired token), full-page for invalid/expired reset links with "Request new link" action.

### Acceptance criteria

- [ ] User registers → receives verification email → verifies → can log in
- [ ] Unverified JWT rejected on all routes except verify/resend
- [ ] Login returns access token (15 min); refresh cookie set httpOnly SameSite=Strict
- [ ] `POST /auth/refresh` issues new access token; TanStack Query retries failed 401s transparently
- [ ] Forgot-password always returns 200 (no email enumeration); reset link expires in 15 min, single-use
- [ ] Logout invalidates current refresh token; logout-all invalidates all tokens for user
- [ ] All auth endpoints return RFC 7807 JSON on error
- [ ] CI passes: cargo fmt, clippy -D warnings, cargo test, pnpm tsc --noEmit, pnpm vitest run
- [ ] Docker Compose brings up app + Postgres; migrations run before app via `depends_on: service_completed_successfully`

---

## Phase 2: Document Management + Basic Editor

**User stories:** 13–20, 22 (welcome doc), 69–72 scoped to doc management

### What to build

End-to-end doc slice: authenticated user can create, rename, list, and delete documents, then open one in a CodeMirror editor (no real-time sync yet — local edits only, auto-saved via debounced PATCH).

Postgres migrations for `documents` and `document_members`. Documents API: `POST /documents`, `GET /documents` (cursor-paginated), `GET /documents/:id`, `PATCH /documents/:id`, `DELETE /documents/:id`. OpenAPI codegen pipeline wired (`make gen`, CI stale-spec check).

Frontend: sidebar with infinite-scroll doc list (useInfiniteQuery), "New Document" button + `Cmd+N`, CodeMirror 6 editor (no Yjs yet), inline editable title above editor, title debounce → `PATCH /documents/:id`. Last-modified timestamp in sidebar.

First login after email verify: auto-create "Welcome to Drafthouse" doc pre-filled with shortcuts/features.

Error/empty states: "No documents yet" + create button when list empty; doc-not-found full-page with back-to-dashboard button; delete confirmation dialog.

### Acceptance criteria

- [ ] `POST /documents` creates doc with title "Untitled"; response navigates to editor with title focused
- [ ] `GET /documents` returns cursor-paginated list; frontend loads next page on scroll
- [ ] Title edit above editor triggers debounced `PATCH /documents/:id`; sidebar reflects update
- [ ] `DELETE /documents/:id` succeeds for owner only (403 for non-owner)
- [ ] Sidebar shows last-modified timestamp per doc
- [ ] `make gen` produces `openapi.json` + typed TS client; CI fails if spec is stale
- [ ] Welcome doc auto-created on first post-verification login
- [ ] Empty state renders "No documents yet" + create CTA
- [ ] Doc-not-found renders error page with dashboard link

---

## Phase 3: Real-Time Collaboration Core

**User stories:** 23–24, 30–34

### What to build

End-to-end collab slice: two authenticated users on the same document see each other's edits appear in real-time via CRDT. Edits survive offline periods and sync on reconnect.

Scylla migrations: `ops` table (WAL, 7-day TTL) and `snapshots` table (ring buffer, version 1–5). Collab service: `DocStore` (DashMap), `DocRoom` (Arc<RwLock<yrs::Doc>> + AtomicUsize connections + last_empty_at), background eviction sweep (60s interval, 5-min idle threshold, final snapshot on evict).

WS ticket endpoint: `POST /documents/:id/ws-ticket` in documents service (token stored hashed, 30s TTL). Collab networking: WS upgrade at `/collab/:doc_id?ticket=<token>`, ticket validated + burned. y-websocket binary protocol: sync-step-1, sync-step-2, update, awareness relay. WAL writes async (100ms buffer). Snapshot trigger: 100 ops OR 30s. On restart: load latest snapshot → replay WAL since `snapshot.taken_at`. SHA256 checksum per snapshot, verified on load. `catch_unwind` around `apply_update`; 100KB message size limit; 1MB doc size limit.

Frontend: replace plain CodeMirror with y-codemirror.next + Yjs Doc. WS provider with exponential backoff + jitter (30s cap). Connection status in toolbar: green/yellow/red dot + text. "Working offline — changes saved locally" when disconnected.

Error/empty states: toast "Working offline" on disconnect; toast "Reconnecting…" during backoff; brief sync spinner after reconnect clears.

### Acceptance criteria

- [ ] Two browser tabs on same doc: edits from tab A appear in tab B within 100ms (p99)
- [ ] WAL ops written to Scylla before client ACK; snapshot taken every 100 ops or 30s
- [ ] Server restart: doc reloads from latest snapshot + WAL replay; no data loss beyond 100ms buffer
- [ ] Corrupt/malicious update caught by `catch_unwind`; only that client disconnects; doc survives
- [ ] Messages >100KB rejected; doc state >1MB blocks further updates
- [ ] Offline edits (tab disconnected) sync automatically on reconnect; both clients converge
- [ ] Connection status indicator reflects connected / connecting / disconnected states
- [ ] WS ticket: 30s expiry enforced; ticket burned on first use; reuse rejected
- [ ] Background eviction: idle room (5min empty) flushed to Scylla and removed from DocStore

---

## Phase 4: Presence & Awareness

**User stories:** 21 (sidebar avatars), 25–29, 35

### What to build

Awareness slice: editors see each other's cursor positions as colored carets with floating name labels, and an avatar strip in the toolbar. Editor cap enforced at WS upgrade.

Collab service: awareness message relay (broadcast to all room members). 8-color palette assigned by join order in `DocRoom`. Idle tracking: awareness entry grayed after 30s no movement, removed after 5min.

Frontend: y-codemirror.next awareness extension renders colored carets + name labels at peer cursor positions. Name labels fade after 3s of that peer's inactivity. Toolbar avatar strip: max 5 shown, +N overflow badge. Idle avatars gray out. Sidebar shows avatars of active editors per doc (from awareness state surfaced via WS events).

Editor cap: atomic check + increment of `DocRoom.connections` at WS upgrade; reject HTTP 429 if ≥ 100.

Error/empty states: no specific error UI needed; cap rejection is a 429 before WS upgrade so standard error toast applies.

### Acceptance criteria

- [ ] Each connected editor sees colored caret at every other editor's cursor position
- [ ] Name label floats above caret; fades after 3s of that editor's inactivity
- [ ] Colors assigned by join order from 8-color palette; consistent within a session
- [ ] Toolbar avatar strip shows up to 5 avatars + "+N" overflow for more
- [ ] Avatar grays out after 30s idle; removed from awareness after 5min
- [ ] Sidebar doc entries show avatars of currently active editors
- [ ] 101st WS connection attempt to same doc receives HTTP 429 at upgrade
- [ ] `TitleUpdated` event from documents service broadcasts `title_update` WS message to all room members; other editors see title change without polling

---

## Phase 5: Sharing, Permissions & Public Access

**User stories:** 36–47

### What to build

Sharing slice: document owner can invite collaborators via link or manage members directly; public docs are viewable by anyone without an account.

Postgres migration: `invite_links` table (token, role, expires_at nullable, max_uses nullable, use_count). Documents API additions: `GET /documents/:id/members`, `DELETE /documents/:id/members/:uid`, `POST /documents/:id/invites`, `GET /documents/:id/invites`, `DELETE /documents/:id/invites/:token`, `POST /invites/:token/accept` (atomic use_count++), `GET /documents/:id/public` (no auth), `PATCH /documents/:id` (is_public toggle).

Permission enforcement: owner/editor/viewer roles checked in documents/core (REST) and collab/core (WS ops). Public doc WS: unauthenticated connection without ticket allowed; server rejects any op messages from that connection (viewer enforced server-side).

Frontend: share modal (members list + roles, invite link generator with role/expiry/max-uses controls, copy button, is_public toggle). Public doc route: read-only CodeMirror (WS active, no editing), "Sign up to edit" banner for unauthenticated users.

Error/empty states: expired/exhausted invite link → full-page error with "Request new link from owner" message; removing yourself as owner blocked with explanatory toast; revoking in-flight invite shows optimistic removal.

### Acceptance criteria

- [ ] Owner generates invite link with editor or viewer role; optional expiry + max_uses
- [ ] Accepting invite link adds user to `document_members`; use_count increments atomically
- [ ] Expired or max-uses-reached invite returns 410 with RFC 7807 body
- [ ] Owner can remove any member; editor/viewer cannot manage members (403)
- [ ] `is_public=true`: unauthenticated user can view via `GET /documents/:id/public` and connect to WS
- [ ] Unauthenticated WS viewer receives sync updates; any op message rejected server-side
- [ ] Frontend shows "Sign up to edit" banner for unauthenticated users on public doc
- [ ] Share modal lists all members with roles; owner can revoke invite links

---

## Phase 6: Editor UX

**User stories:** 48–61

### What to build

Full editor experience slice: toolbar, preview, keyboard shortcuts, command palette, focus mode, font picker, theme, mobile handling.

Fixed toolbar buttons: H1, H2, H3, Bold, Italic, Strikethrough, code block, checklist, divider, preview toggle, connection status. Floating toolbar appears on text selection: Bold, Italic, Strikethrough, inline code, insert link. Both toolbars emit CodeMirror commands.

Preview toggle (`Cmd+Shift+P`): swaps CodeMirror for `markdown-it` rendered HTML. Not split-pane.

Keyboard shortcuts: Cmd+N (new doc), Cmd+S (manual save indicator), Cmd+P (command palette), Cmd+\ (sidebar toggle), Cmd+Shift+P (preview), Cmd+B/I/K/E (formatting), Cmd+Shift+F (focus mode), Esc (exit focus mode).

Command palette: fuzzy search over user's doc titles + actions. Opens with Cmd+P.

Focus mode: sidebar collapsed, toolbar minimal, all chrome hidden. Esc exits.

Font picker: Inter / Lora / JetBrains Mono via Fontsource (self-hosted). Preference in Zustand + localStorage.

Theme: next-themes light/dark/system. Toggle in settings (Phase 7) but wired here. CSS variables on `class="dark"`.

Mobile: `@media` + `navigator.maxTouchPoints` → preview mode default. Edit button shows toast "Editing not supported on mobile". WS still connects (read-only Yrs sync active).

Error/empty states: command palette "No documents found" empty state; rate-limit toast "Too many requests, slow down".

### Acceptance criteria

- [ ] Fixed toolbar buttons insert correct markdown syntax via CodeMirror commands
- [ ] Floating toolbar appears on text selection; applies formatting to selection
- [ ] Preview toggle switches between CodeMirror and markdown-it rendered view
- [ ] All documented keyboard shortcuts work in editor context
- [ ] Command palette opens with Cmd+P; fuzzy searches doc titles; navigates on select
- [ ] Focus mode hides sidebar + chrome; Esc and Cmd+Shift+F toggle it
- [ ] Font choice persists in localStorage; applied to CodeMirror on load
- [ ] Theme defaults to system preference; respects dark/light class on html element
- [ ] Mobile: preview mode default; edit button shows unsupported toast; WS active

---

## Phase 7: Settings, Appearance & GDPR

**User stories:** 62–68

### What to build

Settings slice: authenticated user can manage account and appearance from a single page at `/settings`.

Settings page: shadcn Tabs — Account (email display, change password, delete account with confirmation dialog, export data) and Appearance (theme toggle, font picker — synced with Phase 6 Zustand store).

Change password: new authenticated endpoint (or reuse reset flow — verify current password, set new one). Delete account: `DELETE /auth/me` → Postgres cascade delete + async Scylla doc purge via in-process event. Export: `GET /auth/me/export` → fires `ExportRequested` event → async ZIP of user's `.md` files → email via Resend with download link.

Error/empty states: delete account confirmation dialog (type "delete" to confirm); export triggers toast "Export started — check your email"; wrong current password → inline error on change-password form.

### Acceptance criteria

- [ ] `/settings` accessible from avatar menu in sidebar footer
- [ ] Account tab shows email (read-only); change password form validates current password
- [ ] Delete account requires confirmation dialog; on confirm, all Postgres rows cascade-deleted, Scylla docs purged
- [ ] Export triggers async job; user receives email with ZIP of `.md` files via Resend
- [ ] Appearance tab: theme toggle (light/dark/system) persists in localStorage via next-themes
- [ ] Appearance tab: font picker synced with editor Zustand store

---

## Phase 8: Observability & Deployment Hardening

**Non-story work:** metrics, tracing, backups, load testing, production Docker Compose

### What to build

Observability and deployment slice: the system is monitorable, load-tested, and production-ready on VPS.

Prometheus metrics via `prometheus` crate on `/metrics`: `active_ws_connections` (Gauge), `docs_in_memory` (Gauge), `op_broadcast_latency_ms` (Histogram), `snapshot_duration_ms` (Histogram), `ws_errors_total` (Counter by type), `collab_editor_count` (Histogram). Grafana dashboard in Docker Compose.

Structured logging: `tracing` + `tracing-subscriber`. JSON in prod, pretty in dev. Key events instrumented (editor joined, doc evicted, snapshot taken, WAL flush). Query params redacted from access logs.

Backblaze B2 backup scripts: hourly `nodetool snapshot` for Scylla, daily `pg_dump`, continuous Postgres WAL archiving. Scripts in `scripts/backup/`.

k6 load test: 50 concurrent WebSocket editors on one doc, assert p99 op→broadcast latency < 100ms. Runs pre-deploy only.

Production Docker Compose hardening: resource limits, restart policies, health checks on all services.

### Acceptance criteria

- [ ] `/metrics` returns Prometheus-format metrics for all 6 defined metrics
- [ ] Grafana dashboard visualises all key metrics; accessible at localhost:3001 in Compose
- [ ] Structured JSON logs emitted in prod; key events (join, evict, snapshot) have doc_id + user_id fields
- [ ] k6 script runs against staging; p99 op→broadcast latency < 100ms at 50 concurrent editors
- [ ] B2 backup scripts in place; Scylla hourly + Postgres daily dump + continuous WAL configured
- [ ] All Docker Compose services have health checks and restart: unless-stopped
- [ ] criterion benchmarks for Yrs apply_update + Scylla WAL write run in CI; >2x regression fails build
