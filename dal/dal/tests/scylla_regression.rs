//! Regression tests for the Scylla DAL (docs/BUG_AUDIT.md #28, #33) against
//! a real, disposable ScyllaDB container (testcontainers `scylladb` module).
//!
//! Each `regression_NN_...` test asserts the REQUIRED behavior for audit
//! finding #NN and is intentionally red while the bug is unfixed.

use chrono::{Duration, Utc};
use dal::{ReadLatestSnapshot, ReadOpsSince, ScyllaDescriptor, WriteOp, WriteSnapshot};
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
        // Same bootstrap the migrate-scylla runner performs.
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
        for cql in [
            include_str!("../../../migrations/scylla/0002_create_ops.cql"),
            include_str!("../../../migrations/scylla/0003_create_snapshots.cql"),
        ] {
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

    /// Seed a snapshot row with correct CQL types, bypassing the DAL under
    /// test, so ordering assertions do not depend on the write-path bug.
    async fn seed_snapshot(&self, doc_id: Uuid, version: i32, data: &str, taken_at_ms: i64) {
        self.dal
            .session
            .query_unpaged(
                format!(
                    "INSERT INTO {KEYSPACE}.snapshots (doc_id, version, data, checksum, taken_at) \
                     VALUES (?, ?, ?, ?, ?)"
                ),
                (
                    doc_id,
                    version,
                    data.as_bytes().to_vec(),
                    format!("checksum-{data}"),
                    CqlTimestamp(taken_at_ms),
                ),
            )
            .await
            .unwrap();
    }

    /// Seed a WAL op row with correct CQL types (see audit #33).
    async fn seed_op(&self, doc_id: Uuid, data: Vec<u8>, created_at_ms: i64) {
        self.dal
            .session
            .query_unpaged(
                format!(
                    "INSERT INTO {KEYSPACE}.ops (doc_id, created_at, op_id, client_id, data) \
                     VALUES (?, ?, ?, ?, ?)"
                ),
                (
                    doc_id,
                    CqlTimestamp(created_at_ms),
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                    data,
                ),
            )
            .await
            .unwrap();
    }
}

/// REG-33 (#33): the production WAL and snapshot write paths must succeed
/// against real ScyllaDB. They currently fail: the DAL binds raw `i64`
/// milliseconds to CQL `timestamp` columns, which the pinned driver rejects
/// (`expected BigInt`), and the error is swallowed upstream — so persistence
/// has silently never worked.
#[tokio::test]
async fn regression_33_wal_and_snapshot_writes_succeed() {
    let env = ScyllaEnv::new().await;
    let doc_id = Uuid::new_v4();

    env.dal
        .write_op(NewCollabOp {
            doc_id,
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
            version: 1,
            data: b"snap".to_vec(),
            checksum: "checksum".to_string(),
            taken_at: Utc::now(),
        })
        .await
        .expect("write_snapshot must succeed against real ScyllaDB");
}

/// REG-28 (#28): after the snapshot ring wraps (slots 1–5, then 1 again),
/// `read_latest_snapshot` must return the NEWEST write, not the highest ring
/// slot. The production query orders by `version DESC`, which returns the
/// stale slot-5 row after the wrap.
#[tokio::test]
async fn regression_28_latest_snapshot_is_newest_write_not_highest_slot() {
    let env = ScyllaEnv::new().await;
    let doc_id = Uuid::new_v4();
    let base = Utc::now() - Duration::minutes(10);
    for n in 1..=6i64 {
        let version = if n <= 5 { n as i32 } else { 1 }; // documented ring buffer
        env.seed_snapshot(
            doc_id,
            version,
            &format!("snap-{n}"),
            (base + Duration::seconds(10 * (n - 1))).timestamp_millis(),
        )
        .await;
    }
    let latest = env
        .dal
        .read_latest_snapshot(doc_id)
        .await
        .expect("read_latest_snapshot must succeed against real ScyllaDB")
        .expect("a snapshot must exist");
    assert_eq!(
        latest.data,
        b"snap-6".to_vec(),
        "latest snapshot must be the newest write, not ring slot 5"
    );
}

/// REG-02 (#2, WAL contract): ops written to the WAL must be readable back
/// in ascending `created_at` order, respecting the `since` boundary, and
/// replaying them onto a fresh Yrs doc must converge to the editor's state —
/// this is the contract crash recovery depends on.
#[tokio::test]
async fn regression_02_wal_replay_contract_on_real_scylla() {
    let env = ScyllaEnv::new().await;
    let doc_id = Uuid::new_v4();

    // Build real incremental Yrs updates: insert "A", then "B".
    let editor = Doc::new();
    let text = editor.get_or_insert_text("content");
    text.insert(&mut editor.transact_mut(), 0, "A");
    let sv_after_a = editor.transact().state_vector();
    let op_a = editor
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    text.insert(&mut editor.transact_mut(), 1, "B");
    let op_b = editor.transact().encode_state_as_update_v1(&sv_after_a);

    let base = Utc::now() - Duration::minutes(10);
    env.seed_op(doc_id, op_a.clone(), base.timestamp_millis())
        .await;
    env.seed_op(
        doc_id,
        op_b.clone(),
        (base + Duration::milliseconds(10)).timestamp_millis(),
    )
    .await;

    // Full replay from before the first op.
    let ops = env
        .dal
        .read_ops_since(doc_id, base - Duration::milliseconds(1))
        .await
        .expect("read_ops_since must succeed against real ScyllaDB");
    assert_eq!(ops.len(), 2);
    assert!(
        ops[0].created_at <= ops[1].created_at,
        "ops must replay in order"
    );
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

    // Boundary filter: only the later op remains after the midpoint.
    let tail = env
        .dal
        .read_ops_since(doc_id, base + Duration::milliseconds(5))
        .await
        .expect("read_ops_since must succeed against real ScyllaDB");
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0].data, op_b);
}

/// Guard: the snapshot ring keeps at most SNAPSHOT_RING_SIZE rows per
/// document — slot reuse must overwrite, not accumulate.
#[tokio::test]
async fn guard_snapshot_ring_retains_at_most_five_rows() {
    let env = ScyllaEnv::new().await;
    let doc_id = Uuid::new_v4();
    let base = Utc::now() - Duration::minutes(10);
    for n in 1..=7i64 {
        let version = ((n - 1) % 5 + 1) as i32; // documented ring 1–5
        env.seed_snapshot(
            doc_id,
            version,
            &format!("snap-{n}"),
            (base + Duration::seconds(10 * n)).timestamp_millis(),
        )
        .await;
    }
    let mut versions: Vec<i32> = env
        .dal
        .session
        .query_unpaged(
            format!("SELECT version FROM {KEYSPACE}.snapshots WHERE doc_id = ?"),
            (doc_id,),
        )
        .await
        .unwrap()
        .into_rows_result()
        .unwrap()
        .rows::<(i32,)>()
        .unwrap()
        .map(|row| row.unwrap().0)
        .collect();
    versions.sort_unstable();
    assert_eq!(versions, vec![1, 2, 3, 4, 5]);
}
