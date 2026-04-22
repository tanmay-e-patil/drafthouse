CREATE TABLE invite_links (
    token      TEXT PRIMARY KEY,
    doc_id     UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    role       member_role NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    max_uses   INT,
    use_count  INT NOT NULL DEFAULT 0,
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ
);

CREATE INDEX idx_invite_links_doc ON invite_links(doc_id);
