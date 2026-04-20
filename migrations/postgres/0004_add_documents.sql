CREATE TABLE documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL DEFAULT 'Untitled',
    is_public BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_documents_owner ON documents(owner_id);
CREATE INDEX idx_documents_updated ON documents(owner_id, updated_at DESC);

CREATE TABLE document_members (
    doc_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'viewer',
    PRIMARY KEY (doc_id, user_id),
    CONSTRAINT document_members_role_check CHECK (role IN ('editor', 'viewer'))
);

CREATE INDEX idx_document_members_user ON document_members(user_id);
