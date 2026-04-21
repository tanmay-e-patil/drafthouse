CREATE TABLE ws_tickets (
    token_hash TEXT PRIMARY KEY,
    doc_id     UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_ws_tickets_expires ON ws_tickets(expires_at);
