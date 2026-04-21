CREATE TABLE IF NOT EXISTS drafthouse.ops (
    doc_id     uuid,
    created_at timestamp,
    op_id      uuid,
    client_id  uuid,
    data       blob,
    PRIMARY KEY (doc_id, created_at, op_id)
) WITH CLUSTERING ORDER BY (created_at ASC, op_id ASC)
  AND default_time_to_live = 604800;
