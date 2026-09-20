use kernel::{CollabOp, CollabSnapshot, NewCollabOp, NewCollabSnapshot};

crate::define_dal_transactions!(
    WriteOp => write_op(new_op: NewCollabOp) -> (),
    ReadOpsAfter => read_ops_after(doc_id: uuid::Uuid, sequence: i64) -> Vec<CollabOp>,
    WriteSnapshot => write_snapshot(new_snapshot: NewCollabSnapshot) -> (),
    ReadLatestSnapshot => read_latest_snapshot(doc_id: uuid::Uuid) -> Option<CollabSnapshot>,
    ReadAllSnapshots => read_all_snapshots(doc_id: uuid::Uuid) -> Vec<CollabSnapshot>,
    DeleteSnapshot => delete_snapshot(doc_id: uuid::Uuid, generation: i64) -> (),
    PurgeCollabData => purge_collab_data(doc_id: uuid::Uuid) -> ()
);
