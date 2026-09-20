use super::ScyllaDescriptor;
use crate::collab_txs::{
    DeleteSnapshot, ReadAllSnapshots, ReadLatestSnapshot, ReadOpsAfter, WriteOp, WriteSnapshot,
};
use chrono::{DateTime, Utc};
use dal_tx_impl::impl_transaction;
use kernel::{CollabOp, CollabSnapshot, NewCollabOp, NewCollabSnapshot};
use scylla::frame::value::CqlTimestamp;
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};
use uuid::Uuid;

fn scylla_err(msg: &str, e: impl std::fmt::Display) -> NanoServiceError {
    NanoServiceError::new(
        format!("{}: {}", msg, e),
        NanoServiceErrorStatus::InternalServerError,
    )
}

#[impl_transaction(ScyllaDescriptor, WriteOp, write_op)]
async fn write_op(&self, new_op: NewCollabOp) -> Result<(), NanoServiceError> {
    let created_at = CqlTimestamp(new_op.created_at.timestamp_millis());
    self.session
        .query_unpaged(
            format!(
                "INSERT INTO {}.ops_v2 (doc_id, sequence, op_id, client_id, data, created_at) VALUES (?, ?, ?, ?, ?, ?)",
                self.keyspace
            ),
            (
                new_op.doc_id,
                new_op.sequence,
                new_op.op_id,
                new_op.client_id,
                new_op.data.as_slice(),
                created_at,
            ),
        )
        .await
        .map_err(|e| scylla_err("Failed to write op", e))?;
    Ok(())
}

#[impl_transaction(ScyllaDescriptor, ReadOpsAfter, read_ops_after)]
async fn read_ops_after(
    &self,
    doc_id: Uuid,
    sequence: i64,
) -> Result<Vec<CollabOp>, NanoServiceError> {
    let result = self
        .session
        .query_unpaged(
            format!(
                "SELECT doc_id, sequence, op_id, client_id, data, created_at FROM {}.ops_v2 WHERE doc_id = ? AND sequence > ?",
                self.keyspace
            ),
            (doc_id, sequence),
        )
        .await
        .map_err(|e| scylla_err("Failed to read ops", e))?;

    let rows = result
        .into_rows_result()
        .map_err(|e| scylla_err("Failed to parse rows", e))?;
    let mut ops = Vec::new();
    for row in rows
        .rows::<(Uuid, i64, Uuid, Uuid, Vec<u8>, CqlTimestamp)>()
        .map_err(|e| scylla_err("Failed to deserialize ops", e))?
    {
        let (doc_id, sequence, op_id, client_id, data, created_at) =
            row.map_err(|e| scylla_err("Failed to read op row", e))?;
        let created_at = DateTime::from_timestamp_millis(created_at.0).unwrap_or_else(Utc::now);
        ops.push(CollabOp {
            doc_id,
            sequence,
            created_at,
            op_id,
            client_id,
            data,
        });
    }
    Ok(ops)
}

#[impl_transaction(ScyllaDescriptor, WriteSnapshot, write_snapshot)]
async fn write_snapshot(&self, new_snapshot: NewCollabSnapshot) -> Result<(), NanoServiceError> {
    let taken_at = CqlTimestamp(new_snapshot.taken_at.timestamp_millis());
    self.session
        .query_unpaged(
            format!(
                "INSERT INTO {}.snapshots_v2 (doc_id, generation, through_sequence, data, checksum, taken_at) VALUES (?, ?, ?, ?, ?, ?)",
                self.keyspace
            ),
            (
                new_snapshot.doc_id,
                new_snapshot.generation,
                new_snapshot.through_sequence,
                new_snapshot.data.as_slice(),
                &new_snapshot.checksum,
                taken_at,
            ),
        )
        .await
        .map_err(|e| scylla_err("Failed to write snapshot", e))?;
    Ok(())
}

#[impl_transaction(ScyllaDescriptor, ReadLatestSnapshot, read_latest_snapshot)]
async fn read_latest_snapshot(
    &self,
    doc_id: Uuid,
) -> Result<Option<CollabSnapshot>, NanoServiceError> {
    let result = self
        .session
        .query_unpaged(
            format!(
                "SELECT doc_id, generation, through_sequence, data, checksum, taken_at FROM {}.snapshots_v2 WHERE doc_id = ? ORDER BY generation DESC LIMIT 1",
                self.keyspace
            ),
            (doc_id,),
        )
        .await
        .map_err(|e| scylla_err("Failed to read snapshot", e))?;

    let rows = result
        .into_rows_result()
        .map_err(|e| scylla_err("Failed to parse rows", e))?;
    let mut iter = rows
        .rows::<(Uuid, i64, i64, Vec<u8>, String, CqlTimestamp)>()
        .map_err(|e| scylla_err("Failed to deserialize snapshot", e))?;
    if let Some(row) = iter.next() {
        let (doc_id, generation, through_sequence, data, checksum, taken_at) =
            row.map_err(|e| scylla_err("Failed to read snapshot row", e))?;
        let taken_at = DateTime::from_timestamp_millis(taken_at.0).unwrap_or_else(Utc::now);
        return Ok(Some(CollabSnapshot {
            doc_id,
            generation,
            through_sequence,
            data,
            checksum,
            taken_at,
        }));
    }
    Ok(None)
}

#[impl_transaction(ScyllaDescriptor, ReadAllSnapshots, read_all_snapshots)]
async fn read_all_snapshots(&self, doc_id: Uuid) -> Result<Vec<CollabSnapshot>, NanoServiceError> {
    let result = self
        .session
        .query_unpaged(
            format!(
                "SELECT doc_id, generation, through_sequence, data, checksum, taken_at FROM {}.snapshots_v2 WHERE doc_id = ?",
                self.keyspace
            ),
            (doc_id,),
        )
        .await
        .map_err(|e| scylla_err("Failed to read snapshots", e))?;

    let rows = result
        .into_rows_result()
        .map_err(|e| scylla_err("Failed to parse rows", e))?;
    let mut snapshots = Vec::new();
    for row in rows
        .rows::<(Uuid, i64, i64, Vec<u8>, String, CqlTimestamp)>()
        .map_err(|e| scylla_err("Failed to deserialize snapshots", e))?
    {
        let (doc_id, generation, through_sequence, data, checksum, taken_at) =
            row.map_err(|e| scylla_err("Failed to read snapshot row", e))?;
        let taken_at = DateTime::from_timestamp_millis(taken_at.0).unwrap_or_else(Utc::now);
        snapshots.push(CollabSnapshot {
            doc_id,
            generation,
            through_sequence,
            data,
            checksum,
            taken_at,
        });
    }
    Ok(snapshots)
}

#[impl_transaction(ScyllaDescriptor, DeleteSnapshot, delete_snapshot)]
async fn delete_snapshot(&self, doc_id: Uuid, generation: i64) -> Result<(), NanoServiceError> {
    self.session
        .query_unpaged(
            format!(
                "DELETE FROM {}.snapshots_v2 WHERE doc_id = ? AND generation = ?",
                self.keyspace
            ),
            (doc_id, generation),
        )
        .await
        .map_err(|e| scylla_err("Failed to delete snapshot", e))?;
    Ok(())
}
