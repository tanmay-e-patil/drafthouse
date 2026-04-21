use chrono::DateTime;
use chrono::Utc;
use kernel::{CollabOp, CollabSnapshot, NewCollabOp, NewCollabSnapshot};

crate::define_dal_transactions!(
    WriteOp => write_op(new_op: NewCollabOp) -> (),
    ReadOpsSince => read_ops_since(doc_id: uuid::Uuid, since: DateTime<Utc>) -> Vec<CollabOp>,
    WriteSnapshot => write_snapshot(new_snapshot: NewCollabSnapshot) -> (),
    ReadLatestSnapshot => read_latest_snapshot(doc_id: uuid::Uuid) -> Option<CollabSnapshot>,
    ReadAllSnapshots => read_all_snapshots(doc_id: uuid::Uuid) -> Vec<CollabSnapshot>,
    DeleteSnapshot => delete_snapshot(doc_id: uuid::Uuid, version: i32) -> ()
);
