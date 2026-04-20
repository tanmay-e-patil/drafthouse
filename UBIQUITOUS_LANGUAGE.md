# Ubiquitous Language

## Documents

| Term | Definition | Aliases to avoid |
|---|---|---|
| **Document** | A markdown file owned by one User, editable in real-time by Members | File, note, page |
| **Title** | The human-readable name of a Document, stored in Postgres and synced to active editors via `TitleUpdated` event | Name, heading |
| **Public Document** | A Document with `is_public=true`; readable by unauthenticated users without a WS Ticket | Shared doc, open doc |

## People & Access

| Term | Definition | Aliases to avoid |
|---|---|---|
| **User** | An authenticated identity with email + password stored in Postgres | Account, login, client |
| **Owner** | A User who created the Document; has full control. Derived from `documents.owner_id`, not stored in `document_members` | Admin, creator |
| **Editor** | A Member with read + write access, but no management rights | Collaborator, contributor |
| **Viewer** | A Member with read-only access. Also the implicit role for unauthenticated users on a Public Document | Reader, guest |
| **Member** | A User with any role (Owner, Editor, or Viewer) on a specific Document | Participant, collaborator |

## Authentication & Tokens

| Term | Definition | Aliases to avoid |
|---|---|---|
| **Access Token** | Short-lived JWT (15min) used to authorize REST API requests; stored in memory only | Session token, auth token |
| **Refresh Token** | Long-lived token (30 days) stored hashed in Postgres; delivered as `httpOnly` + `SameSite=Strict` cookie | Session, login cookie |
| **WS Ticket** | One-time URL-safe token (30s TTL) exchanged for a WebSocket connection; burned on first use | WebSocket token, collab ticket |
| **Invite Link** | A shareable URL containing an Invite Token that grants a specific role on a Document | Share link, invite URL |
| **Invite Token** | The random URL-safe string embedded in an Invite Link; may have max uses and/or expiry | Invite code, invite key |

## Real-Time Collaboration

| Term | Definition | Aliases to avoid |
|---|---|---|
| **CRDT** | Conflict-free Replicated Data Type; the algorithm (via Yrs) that merges concurrent edits without conflicts | Operational transform, OT |
| **Op** | A single binary Yrs update representing one or more document changes; the unit written to the WAL | Operation, delta, diff, patch |
| **WAL** | The append-only ScyllaDB `ops` table storing Ops with a 7-day TTL; used to replay changes after a crash or restart | Op log, change log. Note: distinct from PostgreSQL WAL used for DB backups |
| **Snapshot** | A full serialized Yrs Doc state written to ScyllaDB; last 5 retained per Document in a ring buffer | Checkpoint, dump. Note: distinct from `nodetool snapshot` used for ScyllaDB backups |
| **DocRoom** | In-memory struct holding one active Document's Yrs Doc and connection count; one per active Document per process | Room, session, doc session |
| **DocStore** | The `DashMap<DocId, DocRoom>` holding all active DocRooms in process memory | In-memory store, doc cache |
| **Awareness** | Yrs sub-protocol sharing cursor positions and user presence over the same WebSocket connection | Presence, cursor state |
| **Eviction** | Removing an inactive DocRoom from DocStore after 5min empty; triggers a final Snapshot before removal | Expiry, TTL removal |

## Architecture

| Term | Definition | Aliases to avoid |
|---|---|---|
| **Nanoservice** | A feature module split into three crates (networking / core / dal) but deployed as part of a single binary | Microservice, module |
| **Networking layer** | The crate holding HTTP/WS handlers and Actix-web route registration for a nanoservice | Controller, handler crate |
| **Core layer** | The crate holding business logic, validation, and orchestration for a nanoservice; no direct DB access | Service layer, business layer |
| **DAL** | Data Access Layer — the crate holding SQL/CQL queries and DB interactions for a nanoservice | Repository, DAO |
| **Monolith** | The single deployed binary (`ingress/src/main.rs`) that registers routes from all nanoservice networking crates | App, server |
| **dal/kernel** | The shared Rust crate defining all domain structs; the authoritative source of domain types | Domain models, shared types |

## GDPR

| Term | Definition | Aliases to avoid |
|---|---|---|
| **Right to Erasure** | `DELETE /auth/me` — cascades deletes all User data from Postgres and purges their Documents from ScyllaDB | Account deletion, hard delete |
| **Right to Portability** | `GET /auth/me/export` — triggers async ZIP of all the User's Documents as `.md` files, delivered via email | Data export, download |

---

## Relationships

- A **Document** belongs to exactly one **Owner**
- A **Document** has zero or more **Members** (Editors or Viewers); the Owner is not in `document_members`
- An **Invite Link** belongs to one **Document** and grants exactly one role (**Editor** or **Viewer**)
- A **DocRoom** is the live runtime representation of one **Document**; it exists only while at least one editor is active or within 5min of all editors leaving
- A **Snapshot** is a point-in-time capture of a **DocRoom**'s Yrs Doc state; up to 5 kept per Document
- **Ops** in the **WAL** fill the gap between the latest **Snapshot** and the present; loaded on restart to replay changes
- A **WS Ticket** is issued per **User** per **Document** connection attempt; it is single-use and expires in 30 seconds

---

## Example dialogue

> **Dev:** "When an **Editor** sends an **Op**, does it hit the **WAL** before the **DocRoom** or after?"
>
> **Domain expert:** "After. The **Op** is applied to the in-memory Yrs Doc in the **DocRoom** first, then written to the **WAL** asynchronously in a 100ms buffer. The client gets its ACK after the in-memory apply — not after the WAL flush."
>
> **Dev:** "So if the process crashes in that 100ms window, we lose the **Op**?"
>
> **Domain expert:** "Yes, up to 100ms of **Ops** can be lost. On restart, we load the latest **Snapshot** and replay all **WAL Ops** since `snapshot.taken_at`. Anything in the buffer that didn't flush is gone — acceptable for a collaborative editor."
>
> **Dev:** "When does **Eviction** happen? After every editor disconnects?"
>
> **Domain expert:** "No — **Eviction** happens 5 minutes after the **DocRoom** goes empty. A background sweep runs every 60 seconds. On evict, a final **Snapshot** is written before the room is dropped."
>
> **Dev:** "If a **Viewer** connects via a **Public Document** without a **WS Ticket**, can they send **Ops**?"
>
> **Domain expert:** "They connect without a ticket, but ops sent by them are rejected server-side. They receive all broadcasts — real-time read is fine. They cannot mutate the **DocRoom**."

---

## Flagged ambiguities

- **"WAL"** is used in two distinct contexts: (1) the Drafthouse **WAL** — the ScyllaDB `ops` table that stores Yrs **Ops** — and (2) the PostgreSQL WAL used for continuous Postgres backup to Backblaze B2. When speaking about document change replay, use **WAL**. When speaking about database disaster recovery, say **Postgres WAL** explicitly.
- **"Snapshot"** similarly has two meanings: (1) a Yrs Doc **Snapshot** in the `snapshots` ScyllaDB table (domain concept), and (2) a `nodetool snapshot` of ScyllaDB for backup purposes (infrastructure concept). Always qualify as **Yrs Snapshot** or **ScyllaDB backup snapshot** if context is ambiguous.
- **"Token"** is overloaded across five concepts: **Access Token**, **Refresh Token**, **WS Ticket**, **Invite Token**, and password-reset/email-verification tokens. Never use "token" alone; always use the specific term.
- **"Session"** has no precise meaning in this system — there is no session object. The closest equivalents are a **Refresh Token** (auth lifecycle) or a **DocRoom** (collaboration lifecycle). Avoid "session" unless scoped to one of these.
- **"Owner"** is a role but is NOT stored in `document_members`. It is derived from `documents.owner_id`. Code that queries "all members" of a document must handle the Owner separately.
