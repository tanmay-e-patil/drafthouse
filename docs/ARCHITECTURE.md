# Drafthouse Architecture

Collaborative markdown editor. Real-time multi-user editing via CRDT. Rust backend (nanoservices), TanStack Start frontend.

---

## Table of Contents

1. [System Overview](#1-system-overview)
2. [Backend Architecture](#2-backend-architecture)
3. [Data Layer](#3-data-layer)
4. [Database Schemas](#4-database-schemas)
5. [API Design](#5-api-design)
6. [WebSocket Protocol](#6-websocket-protocol)
7. [Frontend Architecture](#7-frontend-architecture)
8. [UI/UX](#8-uiux)
9. [Security](#9-security)
10. [Resilience & Backups](#10-resilience--backups)
11. [Observability](#11-observability)
12. [Testing Strategy](#12-testing-strategy)
13. [Repository Structure](#13-repository-structure)
14. [Deployment](#14-deployment)
15. [Feature Decisions](#15-feature-decisions)
16. [Future Considerations](#16-future-considerations)
13. [Deployment](#13-deployment)
14. [Feature Decisions](#14-feature-decisions)
15. [Future Considerations](#15-future-considerations)

---

## 1. System Overview

| Decision | Choice |
|---|---|
| Document type | Markdown |
| Conflict resolution | CRDT via Yrs (Rust port of Yjs) |
| Real-time transport | WebSocket (y-websocket binary protocol) |
| Backend | Rust, nanoservices pattern, monolith deploy |
| Frontend | TanStack Start + Feature-Sliced Design (FSD) |
| Editor component | CodeMirror 6 + `y-codemirror.next` |
| Markdown preview | Toggle mode (edit ↔ preview) via `markdown-it` |

### Nanoservices Pattern

Every feature splits across three crates per service:

| Layer | Responsibility |
|---|---|
| `networking` | HTTP/WS handlers, route registration, Actix-web |
| `core` | Business logic, validation, orchestration |
| `dal` | SQL/CQL queries, DB interactions |

Dependency injection via Rust trait bounds — no DI containers. `dal/kernel` holds all shared domain structs. Concrete DB descriptors (`SqlxPostGresDescriptor`, `ScyllaDescriptor`) injected once at route registration.

### Services

Three nanoservices, deployed as a single monolith binary:

| Service | Responsibility | DB |
|---|---|---|
| `auth` | Register, login, JWT, password reset, email verify, GDPR export | Postgres |
| `documents` | Doc metadata, permissions, invite links, WS tickets | Postgres |
| `collab` | WebSocket, Yrs CRDT sync, awareness, snapshot orchestration | ScyllaDB |

`ingress/src/main.rs` is the single binary entry point — registers all routes from all networking crates into one Actix-web server.

### Cross-Service Communication

In-process event system (`#[subscribe_to_event]` + `publish_event!` macros). No external broker. Tokio-based, fire-and-forget. Used for:

- `TitleUpdated { doc_id, title }` — documents → collab (broadcasts to WS room)
- `ExportRequested { user_id, email }` — auth → export handler (builds ZIP, sends email)

---

## 2. Backend Architecture

### Active Document State

```rust
struct DocRoom {
    doc: Arc<RwLock<yrs::Doc>>,
    connections: AtomicUsize,
    last_empty_at: Option<Instant>,
}

type DocStore = DashMap<DocId, DocRoom>;
```

One `DocRoom` per active document, held in process memory. All WebSocket connections for a document share one `Arc<RwLock<Doc>>`.

**Eviction:** Background task sweeps `DocStore` every 60 seconds. Evicts rooms where `last_empty_at > 5 minutes`. On evict: flush final snapshot to ScyllaDB first.

**Scaling:** Sticky routing by `doc_id` hash at load balancer. All editors for a document land on the same process. Add processes to add capacity — no cross-process coordination needed.

### Snapshot Strategy

- **Trigger:** every 100 ops OR every 30 seconds (whichever comes first)
- **WAL writes:** async, buffered per 100ms — written to ScyllaDB before ACK to client
- **Retention:** last 5 snapshots per document (ring buffer, version 1-5)
- **Checksum:** SHA256 stored with each snapshot, verified on load
- **On restart:** load latest snapshot → replay WAL ops since `snapshot.taken_at`

### Editor Cap

100 concurrent editors per document maximum. Enforced in `collab/core` at WebSocket upgrade: atomic check + increment of `DocRoom.connections`. Reject with HTTP 429 if ≥ 100.

### Macros (No New Macros)

Existing macros cover all needs:

| Macro | Use |
|---|---|
| `#[impl_transaction]` | Implement DAL trait for `SqlxPostGresDescriptor` or `ScyllaDescriptor` |
| `define_dal_transactions!` | Define DAL traits (works for both Postgres and Scylla) |
| `safe_eject!` | Standardized error conversion |
| `#[api_endpoint]` | HTTP handler wrapper (JWT, RBAC, session cache) |
| `#[subscribe_to_event]` | Register in-process event handler |
| `publish_event!` | Dispatch in-process event |

`ResponseError` trait implemented for `NanoServiceError` — auto-converts to RFC 7807 JSON on every HTTP error. No per-handler boilerplate.

---

## 3. Data Layer

### Databases

| DB | Purpose | Rust Driver |
|---|---|---|
| **PostgreSQL** | Users, auth tokens, doc metadata, permissions, invite links, WS tickets | `sqlx` |
| **ScyllaDB** | Yrs op log (WAL) + doc snapshots | `scylla` crate |

### ScyllaDB Keyspace

```sql
-- VPS (single node)
CREATE KEYSPACE drafthouse
WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1};

-- AWS (multi-node)
CREATE KEYSPACE drafthouse
WITH replication = {'class': 'NetworkTopologyStrategy', 'us-east-1': 3};
```

Keyspace replication strategy configured via `SCYLLA_REPLICATION_FACTOR` env var (Infisical). Migration runner generates correct CQL per environment.

### WAL Cleanup

ScyllaDB ops table has `default_time_to_live = 604800` (7 days). Ops auto-expire — no cleanup code needed. 7 days covers worst-case snapshot interval + disaster recovery window.

### Migrations

- **Postgres:** `sqlx migrate` — migration files in `migrations/postgres/`
- **ScyllaDB:** custom lightweight Rust runner — reads `migrations/scylla/*.cql`, tracks applied in `schema_migrations` table in Scylla
- **When:** CI/CD only (not app startup). Migration containers run before app container via Docker Compose `depends_on: { condition: service_completed_successfully }`
- **Postgres locking:** `sqlx migrate` uses advisory locks — safe for concurrent runners
- **Scylla idempotency:** `IF NOT EXISTS` CQL — safe to run twice

---

## 4. Database Schemas

### PostgreSQL

```sql
CREATE TYPE member_role AS ENUM ('editor', 'viewer');

CREATE TABLE users (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email             TEXT UNIQUE NOT NULL,
    password          TEXT NOT NULL,                    -- argon2id hash
    email_verified_at TIMESTAMPTZ,
    created_at        TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE refresh_tokens (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE password_reset_tokens (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ
);

CREATE TABLE email_verification_tokens (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE documents (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id   UUID REFERENCES users(id) ON DELETE CASCADE,
    title      TEXT NOT NULL DEFAULT 'Untitled',
    is_public  BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE document_members (
    doc_id  UUID REFERENCES documents(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    role    member_role NOT NULL,
    PRIMARY KEY (doc_id, user_id)
);

CREATE TABLE invite_links (
    token      TEXT PRIMARY KEY,                        -- random URL-safe token
    doc_id     UUID REFERENCES documents(id) ON DELETE CASCADE,
    role       member_role NOT NULL,
    expires_at TIMESTAMPTZ,                             -- NULL = never expires
    created_by UUID REFERENCES users(id),
    max_uses   INT,                                     -- NULL = unlimited
    use_count  INT NOT NULL DEFAULT 0
);

CREATE TABLE ws_tickets (
    ticket     TEXT PRIMARY KEY,                        -- random URL-safe token
    doc_id     UUID REFERENCES documents(id) ON DELETE CASCADE,
    user_id    UUID REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL                     -- 30 second expiry
);
```

### ScyllaDB

```sql
-- Op log (append-only WAL)
CREATE TABLE ops (
    doc_id     UUID,
    created_at TIMEUUID,                                -- natural ordering + dedup
    client_id  UUID,
    data       BLOB,                                    -- raw Yrs update bytes
    PRIMARY KEY (doc_id, created_at)
) WITH CLUSTERING ORDER BY (created_at ASC)
  AND default_time_to_live = 604800;                   -- 7 day TTL

-- Snapshots (last 5 per doc, ring buffer)
CREATE TABLE snapshots (
    doc_id   UUID,
    version  INT,                                       -- cycles 1-5
    data     BLOB,                                      -- full Yrs doc state
    checksum TEXT,                                      -- SHA256
    taken_at TIMESTAMP,
    PRIMARY KEY (doc_id, version)
);
```

---

## 5. API Design

### Style

REST + OpenAPI. `utoipa` crate annotates Rust handlers → generates OpenAPI spec. `openapi-ts` generates typed TypeScript client from spec.

```makefile
# Makefile
gen:
    cargo run --bin generate-spec > openapi.json
    pnpm openapi-ts -i openapi.json -o frontend/shared/api/generated
```

Run `make gen` locally after adding/changing endpoints. CI enforces spec is not stale.

### Error Format

RFC 7807 Problem Details via `ResponseError` trait on `NanoServiceError`:

```json
{
  "type": "https://drafthouse.app/errors/not-found",
  "title": "Document Not Found",
  "status": 404,
  "detail": "Document 123e4567 does not exist or you lack access."
}
```

### Endpoints

#### Auth Service

```
POST   /auth/register                 # email + password → user (sends verification email)
POST   /auth/login                    # email + password → access token + sets refresh cookie
POST   /auth/refresh                  # refresh cookie → new access token
POST   /auth/logout                   # invalidate current refresh token
POST   /auth/logout-all               # invalidate all refresh tokens for user
GET    /auth/me                       # JWT → current user profile
POST   /auth/forgot-password          # send reset email (always 200, prevents enumeration)
POST   /auth/reset-password           # { token, new_password } → 200 or 400
POST   /auth/verify-email             # { token } → mark email verified
POST   /auth/resend-verification      # resend verification email
DELETE /auth/me                       # GDPR: cascade delete all user data
GET    /auth/me/export                # GDPR: trigger async ZIP export → email link
```

#### Documents Service

```
POST   /documents                     # create doc → Document
GET    /documents                     # list user's docs (cursor paginated)
GET    /documents/:id                 # get doc metadata → Document
PATCH  /documents/:id                 # update title, is_public
DELETE /documents/:id                 # delete doc (owner only)

GET    /documents/:id/members         # list members + roles
DELETE /documents/:id/members/:uid    # remove member

POST   /documents/:id/invites         # create invite link → { token }
GET    /documents/:id/invites         # list active invite links
DELETE /documents/:id/invites/:token  # revoke invite link
POST   /invites/:token/accept         # join doc via invite (atomic use_count++)

POST   /documents/:id/ws-ticket       # get one-time WS ticket (30s expiry)
GET    /documents/:id/public          # no auth — metadata if is_public=true
```

#### Pagination

Cursor-based on `GET /documents`:

```
GET /documents?cursor=<uuid>&limit=20

Response:
{
  "data": [...],
  "next_cursor": "<uuid>",
  "has_more": true
}
```

Frontend uses TanStack Query `useInfiniteQuery`.

---

## 6. WebSocket Protocol

### Connection

```
Authenticated:   WS /collab/:doc_id?ticket=<one-time-ticket>
Unauthenticated: WS /collab/:doc_id   (read-only, viewer role)
```

Ticket validated + burned at upgrade in `collab/networking`. Origin header validated against allowlist. Ticket stored hashed in `ws_tickets` table, 30s expiry.

### Message Types (y-websocket binary protocol)

| Type | Direction | Description |
|---|---|---|
| `sync-step-1` | Client → Server | Client state vector |
| `sync-step-2` | Server → Client | Missing ops for client |
| `sync-step-2` | Client → Server | Client's missing ops |
| `update` | Client → Server | New op (binary Yrs bytes) |
| `update` | Server → All | Broadcast to room |
| `awareness` | Client → Server | Cursor pos, user info |
| `awareness` | Server → All | Broadcast awareness |
| `title_update` | Server → All | Title changed via REST |

### Limits

- Per-message size: **100KB** max (reject + disconnect repeat offenders)
- Per-doc size: **1MB** max (reject update if merged doc exceeds limit)
- Concurrent editors: **100 max** per doc (HTTP 429 on upgrade if ≥ 100)

### Malicious Client Protection

1. `std::panic::catch_unwind` around `apply_update` — bad update disconnects client only, doc survives
2. 100KB message size check before parsing
3. SHA256 checksum on every snapshot — verified on load

### Reconnection (Frontend)

Exponential backoff + jitter, 30 second cap. Prevents thundering herd on server restart. `y-websocket` provider handles reconnect + resync automatically. On reconnect: client sends full state vector, server sends missing ops, CRDT merges cleanly.

---

## 7. Frontend Architecture

### Stack

| Tool | Purpose |
|---|---|
| TanStack Start | Framework, routing, SSR |
| Feature-Sliced Design (FSD) | Architecture |
| CodeMirror 6 + `y-codemirror.next` | Collaborative markdown editor |
| TanStack Query | Server state (REST API) |
| Zustand | Client state (editor, connection, auth) |
| `markdown-it` | Markdown rendering (preview mode) |
| Vitest | Unit + integration tests |
| Playwright | E2E tests |

### FSD Structure

```
frontend/
├── app/                              # TanStack Start config, root layout
├── pages/                            # Route components
│   ├── home/
│   ├── document/                     # Editor page
│   └── auth/                         # Login, register, verify, reset
├── widgets/                          # Composed UI blocks
│   ├── editor/                       # CodeMirror + Yjs + toolbar
│   └── presence-bar/                 # Cursor overlays, user avatars
├── features/
│   ├── collab/                       # WS connection, Yrs client, awareness
│   │   ├── api/                      # connect, send/receive ops
│   │   ├── model/                    # local doc state, Yjs doc management
│   │   └── ui/                       # cursor overlays, connection status
│   ├── auth/                         # login, register, JWT, refresh
│   └── documents/                    # create, list, delete, share
├── entities/
│   ├── document/
│   └── user/
└── shared/
    ├── api/                          # openapi-ts generated client
    └── lib/                          # utils, design tokens, fetch wrapper
```

### JWT Refresh

TanStack Query global `retry` handler:
- On 401: call `POST /auth/refresh` → update in-memory access token (Zustand) → TanStack Query auto-retries
- Access token: Zustand store (memory only, never localStorage)
- Refresh token: `httpOnly` + `SameSite=Strict` cookie (server sets via `Set-Cookie`)

### Connection State UI

`y-websocket` provider emits `status` + `sync` events → Zustand store → toolbar indicator:

| State | UI |
|---|---|
| `connected` | Green dot |
| `connecting` | Yellow dot, "Reconnecting..." |
| `disconnected` | Red dot, "Offline — changes saved locally" |
| `syncing` | Brief spinner after reconnect |

### Mobile Strategy

- Mobile detected via `@media` + `navigator.maxTouchPoints`
- Mobile → preview mode by default (rendered markdown via `markdown-it`)
- Edit button → toast: "Editing not supported on mobile"
- WS connection still active: receives updates in real-time (read-only Yrs sync)
- No native app

### Browser Targets

Chrome 90+, Firefox 90+, Edge 90+, Safari 15+. No IE, no Opera Mini.

---

## 8. UI/UX

### Stack

| Tool | Purpose |
|---|---|
| shadcn/ui | Component library (copy-paste, Radix primitives underneath) |
| Tailwind CSS | Styling |
| `next-themes` | Dark/light/system theme switching |
| Fontsource | Self-hosted fonts (no Google Fonts dependency) |
| Sonner | Toast notifications (shadcn recommended) |

### Layout

Two modes, toggled via `Cmd+\` or sidebar button:

- **Dashboard mode** — sidebar open (240px), doc list visible, editor fills remaining width
- **Focus mode** — sidebar collapsed, editor full-width, minimal chrome, `Esc` to exit

Sidebar footer: user avatar → settings menu.

### Theming

Light + dark + system default. CSS variables swap on `class="dark"` (shadcn convention). User preference persisted in localStorage via `next-themes`. Toggle in settings page.

### Typography

| Option | Font | Best for |
|---|---|---|
| Sans (default) | Inter | General writing |
| Serif | Lora | Long-form prose |
| Mono | JetBrains Mono | Technical/code-heavy |

All loaded via Fontsource (self-hosted). User preference persisted in Zustand + localStorage.

### Editor Toolbar

**Fixed toolbar (always visible):**
```
H1  H2  H3  |  B  I  ~~S~~  |  ```  [ ]  |  ───  |  [preview-toggle]  [connection-status]
```

**Floating toolbar (appears on text selection):**
```
B   I   ~~S~~   `code`   [link]
```

Connection status indicator always in fixed toolbar (colored dot + text). Preview toggle always accessible.

### Document List (Sidebar)

```
┌─────────────────────────────┐
│ 📄 Product Roadmap          │
│    2 hours ago  👤👤+1      │
├─────────────────────────────┤
│ 📄 Meeting Notes            │
│    Yesterday                │
└─────────────────────────────┘
```

Cursor-paginated, infinite scroll via `useInfiniteQuery`. Avatars shown only for currently active editors (from Yrs Awareness).

### Presence UI

- **In editor:** colored caret at each user's cursor position. Name label floats above, fades after 3s inactivity.
- **Toolbar:** avatar strip, max 5 shown, +N overflow badge.
- **Colors:** 8-color fixed palette, assigned by join order.
- **Idle:** avatar grays out after 30s no movement. Removed from awareness after 5min.

### Document Creation

1. Click "New Document" in sidebar OR press `Cmd+N`
2. `POST /documents` fires immediately (title = "Untitled")
3. Navigate to editor, title field auto-focused
4. Title is editable `h1` above CodeMirror — not a separate input
5. Title syncs to Postgres on blur/debounce via `PATCH /documents/:id`
6. `TitleUpdated` event broadcasts to other active editors

### Sharing Modal

Opened via "Share" button in toolbar:

```
┌─────────────────────────────────────┐
│ Share "Product Roadmap"         [X] │
├─────────────────────────────────────┤
│ People with access                  │
│ 👤 You (owner)                      │
│ 👤 alice@email.com    Editor  [×]   │
│ 👤 bob@email.com      Viewer  [×]   │
├─────────────────────────────────────┤
│ Invite link                         │
│ [Editor ▾] [Generate Link]          │
│ https://drafthouse.app/i/abc123 [📋]│
│ Expires: Never  Max uses: ∞  [Edit] │
├─────────────────────────────────────┤
│ Public access                       │
│ Anyone with link can view  [toggle] │
└─────────────────────────────────────┘
```

`is_public` toggle fires `PATCH /documents/:id` immediately.

### Onboarding

First login after email verification: auto-create "Welcome to Drafthouse" doc pre-filled with markdown showing keyboard shortcuts, formatting examples, features. User lands directly in editor. Doc deletable like any other.

### Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Cmd/Ctrl+N` | New document |
| `Cmd/Ctrl+S` | Manual save (show "Saved" indicator) |
| `Cmd/Ctrl+P` | Command palette (fuzzy search docs + actions) |
| `Cmd/Ctrl+\` | Toggle sidebar |
| `Cmd/Ctrl+Shift+P` | Toggle preview mode |
| `Cmd/Ctrl+B` | Bold |
| `Cmd/Ctrl+I` | Italic |
| `Cmd/Ctrl+K` | Insert link |
| `Cmd/Ctrl+E` | Inline code |
| `Cmd/Ctrl+Shift+F` | Focus mode |
| `Esc` | Exit focus mode |

**Command palette (`Cmd+P`):** fuzzy search over user's docs + actions. Replaces need for separate search feature in v1.

### Auth Pages

Separate routes: `/login`, `/register`, `/verify-email`, `/forgot-password`, `/reset-password`. Centered card layout, no marketing copy, logo + form + links only. Respects system dark/light preference.

### Error & Empty States

**Transient errors** → Sonner toast (auto-dismiss, stacked, accessible):
- WS disconnected: "Working offline"
- Rate limited: "Too many requests, slow down"
- Server error: "Something went wrong, try again"

**Fatal errors** → full-page with clear action:
- Doc not found / no access: message + back to dashboard button
- Doc load failure: message + retry button

**Empty states:**
- All docs deleted: "No documents yet" + create button
- Command palette no results: "No documents found"
- Public doc, unauthenticated: read-only view + "Sign up to edit" banner

### Settings Page

Accessed via avatar menu in sidebar footer (`/settings`). Single scrollable page, shadcn `Tabs`:

```
Settings
├── Account
│   ├── Email (display only)
│   ├── Change password
│   ├── Delete account (GDPR — with confirmation dialog)
│   └── Export data (GDPR — triggers async ZIP email)
└── Appearance
    ├── Theme (light / dark / system)
    └── Editor font (Sans / Serif / Mono)
```

---

## 9. Security


### Authentication

- **Access token:** JWT, 15 minute expiry, contains `user_id` + `verified` claim
- **Refresh token:** 30 day expiry, stored hashed in `refresh_tokens` table, set as `httpOnly` + `SameSite=Strict` cookie
- **Password hashing:** Argon2id — memory=64MB, iterations=3, parallelism=4
- **Email verification:** required before access. Unverified JWT rejected on all routes except `/auth/verify-email` + `/auth/resend-verification`

### WebSocket Auth

One-time ticket flow:
1. Authenticated client calls `POST /documents/:id/ws-ticket`
2. Server generates random token, stores hashed in `ws_tickets` (30s TTL)
3. Client connects: `WS /collab/:doc_id?ticket=<token>`
4. Server validates + burns ticket at upgrade. Ticket useless after first use.

### Transport Security

- CORS: `actix-cors` — `localhost:3000` dev, `https://drafthouse.app` prod
- Security headers: `Strict-Transport-Security`, `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Content-Security-Policy: default-src 'self'`
- WS origin validation: `Origin` header checked at upgrade, reject if not in allowlist
- Log scrubbing: query params redacted from access logs (prevents ticket leakage)
- Rate limiting: Traefik middleware, per-IP

### Permissions Model

Three roles: `owner`, `editor`, `viewer`. Enforced in `documents/core` + `collab/core`.

| Action | owner | editor | viewer |
|---|---|---|---|
| Edit doc | ✅ | ✅ | ❌ |
| View doc | ✅ | ✅ | ✅ |
| Manage members | ✅ | ❌ | ❌ |
| Create invites | ✅ | ❌ | ❌ |
| Delete doc | ✅ | ❌ | ❌ |

Public docs (`is_public=true`): unauthenticated users get viewer access via WS (read-only, ops rejected server-side).

---

## 10. Resilience & Backups

### Failure Scenarios

| Scenario | RPO | RTO | Mechanism |
|---|---|---|---|
| Process crash | 100ms | Seconds (auto-restart) | Async WAL buffer → Scylla; snapshot + replay on boot |
| ScyllaDB node failure | 1 hour | 30-60 min | Hourly snapshots to B2; restore + replay |
| Postgres failure | Near-zero | 5-15 min | Continuous WAL archiving to B2; PITR restore |
| Full VPS loss | 1hr (Scylla) / near-zero (PG) | 30-60 min | Both DBs restored from B2; redeploy via Compose |
| Network partition | Zero | Automatic | CRDT offline merge; y-websocket auto-reconnect |
| Corrupt snapshot | Zero | Seconds | Last 5 snapshots per doc; WAL replay from prior version |
| Malicious client | Zero | Zero | `catch_unwind` + 100KB limit + SHA256 checksum |

### Backup Schedule

| What | How | Frequency | Destination |
|---|---|---|---|
| ScyllaDB snapshot | `nodetool snapshot` | Hourly | Backblaze B2 |
| Postgres full dump | `pg_dump` | Daily | Backblaze B2 |
| Postgres WAL | `pg_basebackup` + WAL streaming | Continuous | Backblaze B2 |

### WAL Write Strategy

Ops written to ScyllaDB WAL asynchronously, buffered per 100ms before flush. Client ACK sent after in-memory apply. Worst-case data loss on crash: 100ms of ops. Acceptable for collaborative editor (equivalent to ~1 keystroke loss).

---

## 11. Observability

### Logging

`tracing` crate + `tracing-subscriber`. JSON format in prod, pretty format in dev. Structured fields on all key events:

```rust
tracing::info!(doc_id = %doc_id, user_id = %user_id, "editor joined");
tracing::warn!(doc_id = %doc_id, size_bytes = %size, "doc size limit approached");
```

### Metrics

`prometheus` crate exposes `/metrics` endpoint:

| Metric | Type | Description |
|---|---|---|
| `active_ws_connections` | Gauge | Current WebSocket connections |
| `docs_in_memory` | Gauge | Active docs in DashMap |
| `op_broadcast_latency_ms` | Histogram | Time from op receive to broadcast |
| `snapshot_duration_ms` | Histogram | Time to serialize + write snapshot |
| `ws_errors_total` | Counter | WebSocket errors by type |
| `collab_editor_count` | Histogram | Editors per doc distribution |

### Visualization

Prometheus + Grafana in Docker Compose. Grafana dashboard monitors all key metrics.

No distributed tracing at VPS stage. Add OpenTelemetry when moving to multi-service AWS deployment.

---

## 12. Testing Strategy

### Philosophy

- `auth` / `documents` services: **unit-heavy** — pure functions with `MockDb` (generated by `#[impl_transaction]`)
- `collab` service: **integration-heavy** — CRDT bugs hide in mocks; use real Scylla + real Yrs docs

### Backend

| Layer | Tool | Approach |
|---|---|---|
| `auth`/`documents` core | Rust unit tests + `MockDb` | Trait-bound mocks, zero DB |
| CRDT correctness | `proptest` | Property-based: commutativity, idempotency, associativity |
| CRDT scenarios | Manual scenario tests | Known edge cases |
| DAL integration | `testcontainers-rs` | Real Scylla + Postgres per test suite |
| WS networking (single client) | `actix_web::test` | In-process, fast |
| WS networking (multi-client) | `tokio-tungstenite` | 2-5 clients, real server, testcontainers DBs |
| Component benchmarks | `criterion` | Yrs merge, Scylla WAL write, snapshot serialize |
| Load / latency SLA | `k6` | 50 concurrent editors, assert p99 < 100ms |

`k6` runs pre-deploy only (needs real infra). All others run in CI on every PR.

### Frontend

| Layer | Tool | Approach |
|---|---|---|
| FSD model/api layers | Vitest | Unit tests, no DOM |
| E2E collab | Playwright | Two browsers, both edit, assert convergence |
| E2E reconnection | Playwright | Disconnect one browser, reconnect, assert sync |

### CI Pipeline

```
PR opened/updated:
├── cargo fmt --check
├── cargo clippy -- -D warnings
├── cargo test (unit + integration via testcontainers)
├── cargo criterion (benchmark regression check)
├── pnpm tsc --noEmit
├── pnpm vitest run
├── make gen → assert OpenAPI spec not stale
└── pnpm playwright test (headless)

Merge to main:
├── All PR checks (re-run)
├── cargo build --release
├── docker build → push to GHCR
└── Dokploy webhook → migrations → deploy
```

---

## 13. Repository Structure

```
drafthouse/
├── CLAUDE.md                         # Architecture overview, crate map, conventions
├── Cargo.toml                        # Workspace root
├── Makefile                          # gen, dev, test, build targets
├── compose.yml                       # App + Postgres + ScyllaDB + Prometheus + Grafana
├── infisical.json                    # Infisical project config (no secrets)
├── migrations/
│   ├── postgres/                     # sqlx migrate files
│   │   ├── 0001_create_users.sql
│   │   ├── 0002_create_documents.sql
│   │   └── ...
│   └── scylla/                       # custom runner CQL files
│       ├── 0001_create_keyspace.cql
│       ├── 0002_create_ops.cql
│       └── 0003_create_snapshots.cql
├── docs/
│   ├── ARCHITECTURE.md               # this file
│   ├── ARCHITECTURE_QUICK_START.md
│   └── EVENT_SYSTEM.md
├── frontend/                         # TanStack Start (FSD)
│   ├── app/
│   ├── pages/
│   ├── widgets/
│   ├── features/
│   │   ├── collab/
│   │   ├── auth/
│   │   └── documents/
│   ├── entities/
│   └── shared/
│       └── api/generated/            # openapi-ts output (committed)
├── ingress/
│   └── src/main.rs                   # Binary entry point, route registration
├── crates/
│   ├── utils/                        # errors, config, safe_eject!
│   └── dal-tx-impl/                  # #[impl_transaction] proc macro
├── dal/
│   ├── kernel/                       # All domain structs (source of truth)
│   └── dal/                          # Trait definitions + Postgres/Scylla impls
└── nanoservices/
    ├── auth/
    │   ├── networking/
    │   ├── core/
    │   └── dal/
    ├── documents/
    │   ├── networking/
    │   ├── core/
    │   └── dal/
    └── collab/
        ├── networking/
        ├── core/
        └── dal/
```

### CLAUDE.md Hierarchy

- **Root `CLAUDE.md`** — crate map, 3-layer pattern, conventions, where things live
- **Per-nanoservice `CLAUDE.md`** — what this service owns, key files, gotchas
- Keep files under 200 lines — Claude reads whole files
- `dal/kernel` is the domain vocabulary — read kernel = understand entire domain

---

## 14. Deployment

### VPS (Initial)

- **Platform:** Dokploy + Traefik on VPS
- **Containerization:** Docker Compose (`compose.yml`)
- **Registry:** GitHub Container Registry (GHCR)
- **Secrets:** Infisical (injected as env vars at runtime)
- **Rate limiting:** Traefik middleware, per-IP
- **Scaling:** Single process. Sticky routing by `doc_id` hash when adding processes.

### Docker Compose Services

```yaml
services:
  migrate-pg:       # runs sqlx migrate, exits
  migrate-scylla:   # runs custom Scylla runner, exits
  app:              # depends on migrate-pg + migrate-scylla success
  postgres:
  scylla:
  prometheus:
  grafana:
```

### Environment Variables (via Infisical)

```
DATABASE_URL
SCYLLA_NODES
SCYLLA_REPLICATION_FACTOR
JWT_SECRET
JWT_EXPIRY_SECS=900
REFRESH_TOKEN_EXPIRY_DAYS=30
WS_TICKET_EXPIRY_SECS=30
SNAPSHOT_OPS_THRESHOLD=100
SNAPSHOT_INTERVAL_SECS=30
DOC_MAX_BYTES=1048576
DOC_MSG_MAX_BYTES=102400
B2_BUCKET=drafthouse-backups
B2_KEY_ID
B2_APP_KEY
APP_ORIGIN
RESEND_API_KEY
```

### AWS Migration Path

Same codebase. Swap:
- Docker Compose → ECS + ALB
- Single VPS Postgres → RDS Multi-AZ (same `DATABASE_URL`)
- Single VPS ScyllaDB → ScyllaDB 3-node cluster, `NetworkTopologyStrategy` RF=3
- Backblaze B2 → S3
- Dokploy → CodePipeline or GitHub Actions → ECS deploy
- Infisical → AWS Secrets Manager (or keep Infisical)

No application code changes required for AWS migration.

---

## 15. Feature Decisions

### Document Features

| Feature | Decision |
|---|---|
| Document limit per user | None (no monetization) |
| Image support | None in v1. External URLs render via `markdown-it`. |
| Doc history / versioning | Internal snapshots only (not user-facing). v2 feature. |
| Search | Not in v1. |
| Templates | Not in v1. |

### Collaboration Features

| Feature | Decision |
|---|---|
| Presence protocol | Yrs Awareness (same WS connection). Redis pub/sub added for zero-downtime deploys later. |
| Cursor colors | Assigned by Yrs Awareness (random per session) |
| Title sync | `TitleUpdated` event → WS broadcast to room. Immediate. |
| Mobile editing | Read-only. Receives real-time updates. Edit prompt shows signup/unsupported message. |
| Offline editing | CRDT buffers locally. Syncs on reconnect automatically. |

### Auth Features

| Feature | Decision |
|---|---|
| Email verification | Required before access |
| Password reset | Token-based, 15min expiry, single-use, email via Resend |
| Session management | Logout current session + logout-all. No per-session list (v2). |
| 2FA | Not in v1 |
| OAuth / social login | Not in v1 |

### Invite Links

| Feature | Decision |
|---|---|
| Max uses | Optional (`max_uses INT`, NULL = unlimited) |
| Expiry | Optional (`expires_at TIMESTAMPTZ`, NULL = never) |
| Decline invite | No endpoint. Pull model — user ignores link to decline. |
| Roles available | `editor`, `viewer` |

### GDPR

| Feature | Decision |
|---|---|
| Right to erasure | `DELETE /auth/me` — Postgres cascade + Scylla doc purge |
| Right to portability | `GET /auth/me/export` — async ZIP of `.md` files → email via Resend |
| Email provider | Resend + `resend-rs` crate, in `auth` nanoservice |
| Async job mechanism | In-process event system (`ExportRequested` event), no external queue |

### Public Documents

Unauthenticated users on `is_public=true` docs:
- `GET /documents/:id/public` — metadata
- WS connection without ticket — viewer role, ops rejected server-side
- Frontend shows rendered preview + "Sign up to edit" banner

---

## 16. Future Considerations

These were explicitly deferred. Implement when needed:

| Feature | Notes |
|---|---|
| Redis pub/sub for awareness | Needed for zero-downtime rolling deploys (prevents split-brain awareness) |
| Image uploads | Reuse Backblaze B2 (already in stack). `POST /documents/:id/images` → B2 → return URL. |
| Full-text search | Consider Meilisearch or Postgres `tsvector` |
| Per-session management | `GET /auth/sessions`, `DELETE /auth/sessions/:id` |
| OpenTelemetry tracing | Add when extracting services on AWS |
| Workspace/org model | Extend permissions if multi-tenant needed |
| `notifications` nanoservice | Extract from `auth` when multiple email types exist |
| ScyllaDB CDC | For near-zero RPO on Scylla (currently 1hr via hourly snapshots) |
| Native mobile app | React Native or Flutter. Currently mobile = read-only web. |
| Document versioning UI | Expose snapshots as user-visible history |
| 2FA | TOTP via `totp-rs` crate |
| OAuth | Add social login providers |
