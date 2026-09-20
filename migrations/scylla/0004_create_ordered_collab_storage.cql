CREATE TABLE IF NOT EXISTS drafthouse.ops_v2 (
    doc_id     uuid,
    sequence   bigint,
    op_id      uuid,
    client_id  uuid,
    data       blob,
    created_at timestamp,
    PRIMARY KEY (doc_id, sequence)
) WITH CLUSTERING ORDER BY (sequence ASC)
  AND default_time_to_live = 604800;

CREATE TABLE IF NOT EXISTS drafthouse.snapshots_v2 (
    doc_id           uuid,
    generation       bigint,
    through_sequence bigint,
    data             blob,
    checksum         text,
    taken_at         timestamp,
    PRIMARY KEY (doc_id, generation)
) WITH CLUSTERING ORDER BY (generation DESC);
