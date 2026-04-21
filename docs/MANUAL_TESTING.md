# Manual Testing

curl-based smoke tests for auth endpoints. Run against local dev server (`make dev`).

**Base URL:** `http://localhost:8080`

---

## Setup

```bash
# Start Postgres + ScyllaDB
docker compose up postgres scylla -d

# Wait for ScyllaDB to be healthy (~30s)
docker compose ps   # wait until scylla shows "healthy"

# Run Postgres migrations
DATABASE_URL=postgres://drafthouse:drafthouse@localhost:5432/drafthouse \
  sqlx migrate run --source migrations/postgres

# Run ScyllaDB migrations (defaults: 127.0.0.1:9042, keyspace=drafthouse)
cargo run --bin migrate-scylla

# Start backend + frontend
make dev
```

---

## Auth Endpoints

### POST /auth/register

**Happy path → 201**
```bash
curl -s -w "\n%{http_code}" -X POST http://localhost:8080/auth/register \
  -H "Content-Type: application/json" \
  -d '{"email": "alice@example.com", "password": "password123"}'
```
Expected: `201` + `{"user_id": "...", "email": "alice@example.com", "message": "..."}`

---

**Duplicate email → 409**
```bash
curl -s -w "\n%{http_code}" -X POST http://localhost:8080/auth/register \
  -H "Content-Type: application/json" \
  -d '{"email": "alice@example.com", "password": "password123"}'
```
Expected: `409` + RFC 7807 error body

---

**Empty email → 400**
```bash
curl -s -w "\n%{http_code}" -X POST http://localhost:8080/auth/register \
  -H "Content-Type: application/json" \
  -d '{"email": "", "password": "password123"}'
```
Expected: `400`

---

**Short password → 400**
```bash
curl -s -w "\n%{http_code}" -X POST http://localhost:8080/auth/register \
  -H "Content-Type: application/json" \
  -d '{"email": "bob@example.com", "password": "abc"}'
```
Expected: `400`

---

### POST /auth/verify-email

Get the raw verification token from the DB (email delivery skipped in dev unless `RESEND_API_KEY` is set):

```bash
# Grab the raw token hash from DB, then reverse-lookup is impossible —
# instead, check logs or temporarily log the token, OR insert a known token:
TOKEN=$(psql "postgres://drafthouse:drafthouse@localhost:5432/drafthouse" -t -c \
  "SELECT token_hash FROM email_verification_tokens LIMIT 1;" | xargs)
echo "token_hash in DB: $TOKEN"
```

> In dev, register logs a warning when email fails. The raw token is not stored — only its SHA-256 hash. To test verify-email end-to-end locally, either configure a real `RESEND_API_KEY` + `APP_ORIGIN`, or insert a known token directly:

```bash
# Insert a known raw token for the most recently registered user
psql "postgres://drafthouse:drafthouse@localhost:5432/drafthouse" -c "
  INSERT INTO email_verification_tokens (user_id, token_hash, expires_at)
  SELECT id,
         encode(sha256('localtest_token'::bytea), 'hex'),
         now() + interval '24 hours'
  FROM users ORDER BY created_at DESC LIMIT 1;
"

curl -s -w "\n%{http_code}" -X POST http://localhost:8080/auth/verify-email \
  -H "Content-Type: application/json" \
  -d '{"token": "localtest_token"}'
```
Expected: `200` + `{"message": "Email verified successfully..."}`

---

**Invalid token → 400**
```bash
curl -s -w "\n%{http_code}" -X POST http://localhost:8080/auth/verify-email \
  -H "Content-Type: application/json" \
  -d '{"token": "this_token_does_not_exist"}'
```
Expected: `400`

---

**Empty token → 400**
```bash
curl -s -w "\n%{http_code}" -X POST http://localhost:8080/auth/verify-email \
  -H "Content-Type: application/json" \
  -d '{"token": ""}'
```
Expected: `400`

---

### POST /auth/resend-verification

**Already verified → 400**
```bash
curl -s -w "\n%{http_code}" -X POST http://localhost:8080/auth/resend-verification \
  -H "Content-Type: application/json" \
  -d '{"email": "alice@example.com"}'
```
Expected: `400` (if alice is already verified)

---

**Unknown email → 400**
```bash
curl -s -w "\n%{http_code}" -X POST http://localhost:8080/auth/resend-verification \
  -H "Content-Type: application/json" \
  -d '{"email": "ghost@example.com"}'
```
Expected: `400`

---

**Happy path → 200** (requires `RESEND_API_KEY`)
```bash
RESEND_API_KEY=re_xxx APP_ORIGIN=http://localhost:3000 cargo run --bin ingress &

curl -s -w "\n%{http_code}" -X POST http://localhost:8080/auth/resend-verification \
  -H "Content-Type: application/json" \
  -d '{"email": "unverified@example.com"}'
```
Expected: `200`

---

## Collab / WebSocket

> Sharing UI not built yet. Both users must own the same document. For now, test with two tabs logged in as the same user, or manually insert a `doc_members` row (see DB Inspection below).

### Happy path — two tabs, real-time sync

1. Open `http://localhost:3000` in two browser windows
2. Register/login as the same user (or two users with DB access to same doc)
3. Open the same document in both windows
4. Type in window A → text should appear in window B within milliseconds

### POST /documents/:id/ws-ticket

```bash
# Login first to get a token
TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email": "alice@example.com", "password": "password123"}' \
  | jq -r '.access_token')

# Get doc id
DOC_ID=$(curl -s http://localhost:8080/documents \
  -H "Authorization: Bearer $TOKEN" \
  | jq -r '.items[0].id')

# Issue WS ticket
curl -s -w "\n%{http_code}" -X POST http://localhost:8080/documents/$DOC_ID/ws-ticket \
  -H "Authorization: Bearer $TOKEN"
```
Expected: `201` + `{"ticket": "<token>"}`

---

**Ticket reuse → reject**

Use the same ticket a second time for `WS /collab/:doc_id?ticket=<token>` — second upgrade must fail (ticket is single-use, burned on first connection).

---

**Expired ticket → reject**

Wait >30 seconds after issuing ticket, then attempt WS upgrade. Must fail.

---

### Snapshot trigger

Make 100+ edits in the editor (or wait 30 seconds), then inspect ScyllaDB:

```bash
# Connect to ScyllaDB
docker exec -it $(docker compose ps -q scylla) cqlsh

USE drafthouse;
SELECT doc_id, version, created_at FROM snapshots;
```
Expected: at least one row for the edited document.

---

### Reconnect after disconnect

1. Open doc, make edits
2. Close browser tab (disconnect WS)
3. Reopen doc in new tab
4. All edits should be present (replayed from snapshot + WAL)

---

## DB Inspection

```bash
# Connect to Postgres
psql "postgres://drafthouse:drafthouse@localhost:5432/drafthouse"

# Check users
SELECT id, email, email_verified_at, created_at FROM users;

# Check pending verification tokens
SELECT u.email, t.expires_at
FROM email_verification_tokens t
JOIN users u ON u.id = t.user_id;

# Check WS tickets (hashed, burned on use via DELETE)
SELECT doc_id, user_id, expires_at FROM ws_tickets;

# Manually grant second user access to a doc (no sharing UI yet)
INSERT INTO doc_members (doc_id, user_id, role)
VALUES ('<doc_id>', '<user_id>', 'editor');
```

```bash
# Connect to ScyllaDB
docker exec -it $(docker compose ps -q scylla) cqlsh

USE drafthouse;

# Check snapshots (ring buffer, last 5 per doc)
SELECT doc_id, version, created_at FROM snapshots;

# Check WAL ops (7-day TTL, no manual cleanup needed)
SELECT doc_id, seq, created_at FROM ops LIMIT 20;
```
