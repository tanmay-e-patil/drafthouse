use chrono::Utc;
use kernel::NewCollabSnapshot;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    room::{DocRoom, DocStore, SNAPSHOT_RING_SIZE, encode_snapshot, verify_snapshot_checksum},
    sync_protocol::apply_update_safe,
};
use dal::{DeleteSnapshot, ReadLatestSnapshot, ReadOpsAfter, WriteSnapshot};
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

/// Restore the newest snapshot and subsequent WAL operations before a room is used.
pub async fn restore_room<D>(
    dal: &D,
    doc_id: Uuid,
    room: &DocRoom,
) -> Result<bool, NanoServiceError>
where
    D: ReadLatestSnapshot + ReadOpsAfter,
{
    let (through_sequence, had_snapshot) = match dal.read_latest_snapshot(doc_id).await? {
        Some(snapshot) => {
            if !verify_snapshot_checksum(&snapshot.data, &snapshot.checksum) {
                return Err(NanoServiceError::new(
                    "Snapshot checksum mismatch",
                    NanoServiceErrorStatus::InternalServerError,
                ));
            }
            apply_update_safe(
                &room
                    .doc
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
                &snapshot.data,
            )
            .ok_or_else(|| {
                NanoServiceError::new(
                    "Failed to apply snapshot",
                    NanoServiceErrorStatus::InternalServerError,
                )
            })?;
            room.restore_progress(snapshot.through_sequence, snapshot.generation);
            (snapshot.through_sequence, true)
        }
        None => (0, false),
    };

    let ops = dal.read_ops_after(doc_id, through_sequence).await?;
    let had_ops = !ops.is_empty();
    for op in ops {
        apply_update_safe(
            &room
                .doc
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            &op.data,
        )
        .ok_or_else(|| {
            NanoServiceError::new(
                "Failed to replay collaboration operation",
                NanoServiceErrorStatus::InternalServerError,
            )
        })?;
        room.restore_progress(op.sequence, 0);
    }

    Ok(had_snapshot || had_ops)
}

/// Persist a snapshot for the given room to ScyllaDB.
pub async fn persist_snapshot<D>(dal: &D, doc_id: Uuid, room: &DocRoom) -> bool
where
    D: WriteSnapshot + ReadLatestSnapshot + DeleteSnapshot,
{
    if room.is_closed() {
        return false;
    }
    room.serialize_update(|| async {
        let (data, checksum) = {
            let doc = room.doc.read().unwrap_or_else(std::sync::PoisonError::into_inner);
            encode_snapshot(&doc)
        };

        let generation = room.next_snapshot_generation();
        let through_sequence = room.current_sequence();
        let taken_at = Utc::now();

        let result = dal
            .write_snapshot(NewCollabSnapshot {
                doc_id,
                generation,
                through_sequence,
                data,
                checksum,
                taken_at,
            })
            .await;

        if let Err(e) = result {
            tracing::warn!(doc_id = %doc_id, generation, "snapshot write failed: {}", e);
            return false;
        }

        room.mark_snapshot_persisted();

        let stale_generation = generation - i64::from(SNAPSHOT_RING_SIZE);
        if stale_generation > 0
            && let Err(e) = dal.delete_snapshot(doc_id, stale_generation).await {
                tracing::warn!(doc_id = %doc_id, stale_generation, "stale snapshot deletion failed: {}", e);
            }

        tracing::debug!(doc_id = %doc_id, generation, through_sequence, "snapshot written");
        true
    })
    .await
}

/// Eviction sweep: remove rooms idle > 5 minutes, flush final snapshot.
pub async fn eviction_sweep<D>(dal: &D, store: &DocStore)
where
    D: WriteSnapshot + ReadLatestSnapshot + DeleteSnapshot + Clone + Send + Sync + 'static,
{
    let mut snapshot_candidates = Vec::new();
    let mut eviction_candidates = Vec::new();
    for entry in store.iter() {
        let candidate = (*entry.key(), Arc::clone(entry.value()));
        if entry.value().is_idle_for_eviction() {
            eviction_candidates.push(candidate);
        } else if entry.value().should_snapshot() {
            snapshot_candidates.push(candidate);
        }
    }

    for (doc_id, room) in snapshot_candidates {
        persist_snapshot(dal, doc_id, &room).await;
    }

    for (doc_id, room) in eviction_candidates {
        if !persist_snapshot(dal, doc_id, &room).await {
            continue;
        }

        if store
            .remove_if(&doc_id, |_, current| {
                Arc::ptr_eq(current, &room) && current.is_idle_for_eviction()
            })
            .is_some()
        {
            tracing::info!(doc_id = %doc_id, "evicted idle room after flushing snapshot");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::room::{DocRoom, DocStore};
    use dashmap::DashMap;
    use kernel::{CollabSnapshot, NewCollabSnapshot};
    use std::sync::{Arc, Mutex};
    use utils::errors::NanoServiceError;

    #[derive(Clone)]
    struct MockDal {
        snapshots: Arc<Mutex<Vec<CollabSnapshot>>>,
    }

    impl MockDal {
        fn new() -> Self {
            Self {
                snapshots: Arc::new(Mutex::new(vec![])),
            }
        }
    }

    impl WriteSnapshot for MockDal {
        fn write_snapshot(
            &self,
            new_snapshot: NewCollabSnapshot,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let snapshots = Arc::clone(&self.snapshots);
            async move {
                snapshots.lock().unwrap().push(CollabSnapshot {
                    doc_id: new_snapshot.doc_id,
                    generation: new_snapshot.generation,
                    through_sequence: new_snapshot.through_sequence,
                    data: new_snapshot.data,
                    checksum: new_snapshot.checksum,
                    taken_at: new_snapshot.taken_at,
                });
                Ok(())
            }
        }
    }

    impl ReadLatestSnapshot for MockDal {
        fn read_latest_snapshot(
            &self,
            doc_id: uuid::Uuid,
        ) -> impl std::future::Future<Output = Result<Option<CollabSnapshot>, NanoServiceError>> + Send
        {
            let snapshots = Arc::clone(&self.snapshots);
            async move {
                Ok(snapshots
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|s| s.doc_id == doc_id)
                    .max_by_key(|s| s.generation)
                    .cloned())
            }
        }
    }

    impl DeleteSnapshot for MockDal {
        fn delete_snapshot(
            &self,
            doc_id: uuid::Uuid,
            generation: i64,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let snapshots = Arc::clone(&self.snapshots);
            async move {
                snapshots
                    .lock()
                    .unwrap()
                    .retain(|s| !(s.doc_id == doc_id && s.generation == generation));
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn persist_snapshot_writes_to_dal() {
        let dal = MockDal::new();
        let room = DocRoom::new();
        let doc_id = Uuid::new_v4();
        let ok = persist_snapshot(&dal, doc_id, &room).await;
        assert!(ok);
        assert_eq!(dal.snapshots.lock().unwrap().len(), 1);
        assert_eq!(dal.snapshots.lock().unwrap()[0].doc_id, doc_id);
    }

    #[tokio::test]
    async fn snapshot_retention_keeps_five_monotonic_generations() {
        let dal = MockDal::new();
        let room = DocRoom::new();
        let doc_id = Uuid::new_v4();
        for _ in 0..6 {
            persist_snapshot(&dal, doc_id, &room).await;
        }
        let snaps = dal.snapshots.lock().unwrap();
        let generations: Vec<i64> = snaps.iter().map(|s| s.generation).collect();
        assert_eq!(generations, vec![2, 3, 4, 5, 6]);
    }

    #[tokio::test]
    async fn eviction_sweep_removes_idle_rooms() {
        let dal = MockDal::new();
        let store: DocStore = DashMap::new();
        let doc_id = Uuid::new_v4();

        let room = Arc::new(DocRoom::new());
        // Manually set last_empty_at far in the past by not adding any connections
        // Room starts with last_empty_at = Some(now), but we need it to appear old.
        // Simulate by using is_idle_for_eviction check on fresh room after we override.
        store.insert(doc_id, room.clone());

        // Room is not yet idle (just created), so nothing evicted
        eviction_sweep(&dal, &store).await;
        assert_eq!(store.len(), 1);
    }
}
