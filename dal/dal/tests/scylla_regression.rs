//! Regression tests for ordered collaboration persistence against a real,
//! disposable ScyllaDB container.

use chrono::{Duration, Utc};
use dal::{
    DeleteSnapshot, ReadAllSnapshots, ReadLatestSnapshot, ReadOpsAfter, ScyllaDescriptor, WriteOp,
    WriteSnapshot,
};
use kernel::{NewCollabOp, NewCollabSnapshot};
use scylla::frame::value::CqlTimestamp;
use std::sync::Arc;
use testcontainers_modules::{
    scylladb::ScyllaDB,
    testcontainers::{ContainerAsync, runners::AsyncRunner},
};
use uuid::Uuid;
use yrs::{Doc, GetString, ReadTxn, StateVector, Text, Transact, updates::decoder::Decode};

const KEYSPACE: &str = "drafthouse";

struct ScyllaEnv {
    dal: ScyllaDescriptor,
    _container: ContainerAsync<ScyllaDB>,
}

impl ScyllaEnv {
    async fn new() -> Self {
        let container = ScyllaDB::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(9042).await.unwrap();
        let session = scylla::SessionBuilder::new()
            .known_node(format!("127.0.0.1:{port}"))
            .build()
            .await
            .unwrap();
        session
            .query_unpaged(
                format!(
                    "CREATE KEYSPACE IF NOT EXISTS {KEYSPACE} \
                     WITH replication = {{'class': 'SimpleStrategy', 'replication_factor': 1}}"
                ),
                &[],
            )
            .await
            .unwrap();
        for cql in [include_str!(
            "../../../migrations/scylla/0004_create_ordered_collab_storage.cql"
        )] {
            for stmt in cql
                .split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty() && !s.starts_with("--"))
            {
                session.query_unpaged(stmt, &[]).await.unwrap();
            }
        }
        Self {
            dal: ScyllaDescriptor {
                session: Arc::new(session),
                keyspace: KEYSPACE.to_string(),
            },
            _container: container,
        }
    }

    async fn seed_snapshot(
        &self,
        doc_id: Uuid,
        generation: i64,
        through_sequence: i64,
        data: &str,
        taken_at_ms: i64,
    ) {
        self.dal
            .session
            .query_unpaged(
                format!(
                    "INSERT INTO {KEYSPACE}.snapshots_v2 \
                     (doc_id, generation, through_sequence, data, checksum, taken_at) \
                     VALUES (?, ?, ?, ?, ?, ?)"
                ),
                (
                    doc_id,
                    generation,
                    through_sequence,
                    data.as_bytes().to_vec(),
                    format!("checksum-{data}"),
                    CqlTimestamp(taken_at_ms),
                ),
            )
            .await
            .unwrap();
    }

    async fn seed_op(&self, doc_id: Uuid, sequence: i64, data: Vec<u8>) {
        self.dal
            .session
            .query_unpaged(
                format!(
                    "INSERT INTO {KEYSPACE}.ops_v2 \
                     (doc_id, sequence, op_id, client_id, data, created_at) \
                     VALUES (?, ?, ?, ?, ?, ?)"
                ),
                (
                    doc_id,
                    sequence,
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                    data,
                    CqlTimestamp(Utc::now().timestamp_millis()),
                ),
            )
            .await
            .unwrap();
    }
}

/// REG-33 (#33): production WAL and snapshot writes use correct CQL types.
#[tokio::test]
async fn regression_33_wal_and_snapshot_writes_succeed() {
    let env = ScyllaEnv::new().await;
    let doc_id = Uuid::new_v4();

    env.dal
        .write_op(NewCollabOp {
            doc_id,
            sequence: 1,
            op_id: Uuid::new_v4(),
            client_id: Uuid::new_v4(),
            data: b"op".to_vec(),
            created_at: Utc::now(),
        })
        .await
        .expect("write_op must succeed against real ScyllaDB");

    env.dal
        .write_snapshot(NewCollabSnapshot {
            doc_id,
            generation: 1,
            through_sequence: 1,
            data: b"snap".to_vec(),
            checksum: "checksum".to_string(),
            taken_at: Utc::now(),
        })
        .await
        .expect("write_snapshot must succeed against real ScyllaDB");
}

/// REG-28 (#28): latest means greatest monotonic generation, independent of
/// reusable slots, timestamps, or clock skew.
#[tokio::test]
async fn regression_28_latest_snapshot_is_newest_write_not_highest_slot() {
    let env = ScyllaEnv::new().await;
    let doc_id = Uuid::new_v4();
    let now = Utc::now();
    env.seed_snapshot(doc_id, 5, 50, "snap-5", now.timestamp_millis())
        .await;
    env.seed_snapshot(
        doc_id,
        6,
        60,
        "snap-6",
        (now - Duration::hours(1)).timestamp_millis(),
    )
    .await;

    let latest = env
        .dal
        .read_latest_snapshot(doc_id)
        .await
        .expect("read_latest_snapshot must succeed against real ScyllaDB")
        .expect("a snapshot must exist");
    assert_eq!(latest.generation, 6);
    assert_eq!(latest.through_sequence, 60);
    assert_eq!(latest.data, b"snap-6".to_vec());
}

/// REG-02 (#2): WAL replay is ordered by sequence and starts strictly after
/// the snapshot's `through_sequence` boundary.
#[tokio::test]
async fn regression_02_wal_replay_contract_on_real_scylla() {
    let env = ScyllaEnv::new().await;
    let doc_id = Uuid::new_v4();

    let editor = Doc::new();
    let text = editor.get_or_insert_text("content");
    text.insert(&mut editor.transact_mut(), 0, "A");
    let sv_after_a = editor.transact().state_vector();
    let op_a = editor
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    text.insert(&mut editor.transact_mut(), 1, "B");
    let op_b = editor.transact().encode_state_as_update_v1(&sv_after_a);

    env.seed_op(doc_id, 1, op_a.clone()).await;
    env.seed_op(doc_id, 2, op_b.clone()).await;

    let ops = env
        .dal
        .read_ops_after(doc_id, 0)
        .await
        .expect("read_ops_after must succeed against real ScyllaDB");
    assert_eq!(ops.len(), 2);
    assert_eq!(ops[0].sequence, 1);
    assert_eq!(ops[1].sequence, 2);
    assert_eq!(ops[0].data, op_a);
    assert_eq!(ops[1].data, op_b);

    let replica = Doc::new();
    for op in &ops {
        let update = yrs::Update::decode_v1(&op.data).unwrap();
        replica.transact_mut().apply_update(update).unwrap();
    }
    assert_eq!(
        replica
            .get_or_insert_text("content")
            .get_string(&replica.transact()),
        "AB"
    );

    let tail = env
        .dal
        .read_ops_after(doc_id, 1)
        .await
        .expect("read_ops_after must succeed against real ScyllaDB");
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0].sequence, 2);
    assert_eq!(tail[0].data, op_b);
}

/// Guard: snapshot retention can remove generations older than the latest five.
#[tokio::test]
async fn guard_snapshot_ring_retains_at_most_five_rows() {
    let env = ScyllaEnv::new().await;
    let doc_id = Uuid::new_v4();
    for generation in 1..=6 {
        env.seed_snapshot(
            doc_id,
            generation,
            generation * 10,
            &format!("snap-{generation}"),
            Utc::now().timestamp_millis(),
        )
        .await;
    }
    env.dal.delete_snapshot(doc_id, 1).await.unwrap();

    let snapshots = env.dal.read_all_snapshots(doc_id).await.unwrap();
    let generations: Vec<i64> = snapshots
        .into_iter()
        .map(|snapshot| snapshot.generation)
        .collect();
    assert_eq!(generations, vec![6, 5, 4, 3, 2]);
}
