# Manual Testing

curl-based smoke tests for auth endpoints. Run against local dev server (`make dev`).

**Base URL:** `http://localhost:8080`

---

## Setup

```bash
# Start Postgres
docker compose up postgres -d

# Run migrations
DATABASE_URL=postgres://drafthouse:drafthouse@localhost:5432/drafthouse \
  sqlx migrate run --source migrations/postgres

# Start backend
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

## DB Inspection

```bash
# Connect
psql "postgres://drafthouse:drafthouse@localhost:5432/drafthouse"

# Check users
SELECT id, email, email_verified_at, created_at FROM users;

# Check pending verification tokens
SELECT u.email, t.expires_at
FROM email_verification_tokens t
JOIN users u ON u.id = t.user_id;
```
