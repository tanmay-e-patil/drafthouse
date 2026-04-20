# Drafthouse

Collaborative markdown editor. Real-time multi-user editing via CRDT. Rust backend, TanStack Start frontend.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for full design decisions.

---

## Prerequisites

| Tool | Purpose |
|---|---|
| Rust (stable) | Backend |
| `cargo-watch` | `make dev` hot reload |
| Node.js + pnpm | Frontend |
| Docker | Integration tests (testcontainers), local dev stack |

---

## Local Development

```bash
# Start backend (hot reload) + frontend dev server
make dev

# Regenerate OpenAPI TypeScript client after changing endpoints
make gen
```

---

## Testing

### Unit tests (no Docker required)

```bash
# All unit tests across all crates + frontend
make test

# Backend only
cargo test --workspace

# Single crate
cargo test -p auth-core
```

### Integration tests (Docker required)

Integration tests use [testcontainers](https://github.com/testcontainers/testcontainers-rs) — they spin up real Postgres containers automatically. **Docker daemon must be running.**

```bash
# Run all integration tests across the workspace
cargo test --workspace

# Run only auth integration tests
cargo test -p auth-networking --test auth_integration_test

# Run a single test by name
cargo test -p auth-networking --test auth_integration_test -- verify_email_success

# Run with output visible (useful for debugging)
cargo test -p auth-networking --test auth_integration_test -- --nocapture
```

> The `resend_success` test sets a process-level env var and is marked `#[serial]`.
> Run it in isolation if you see flaky env var behavior:
> ```bash
> cargo test -p auth-networking --test auth_integration_test -- resend_success --nocapture
> ```

### Frontend tests

```bash
cd frontend && pnpm test
```

### E2E tests (Playwright)

```bash
cd frontend && pnpm playwright test
```

---

## Full CI check locally

Run the same checks CI runs on every PR:

```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cd frontend && pnpm tsc --noEmit && pnpm test && pnpm playwright test
```
