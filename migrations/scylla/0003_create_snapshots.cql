CREATE TABLE IF NOT EXISTS drafthouse.snapshots (
    doc_id   uuid,
    version  int,
    data     blob,
    checksum text,
    taken_at timestamp,
    PRIMARY KEY (doc_id, version)
);
