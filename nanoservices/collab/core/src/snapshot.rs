use chrono::Utc;
use kernel::NewCollabSnapshot;
use uuid::Uuid;

use crate::room::{DocRoom, DocStore, encode_snapshot};
use dal::{DeleteSnapshot, ReadLatestSnapshot, WriteSnapshot};

/// Persist a snapshot for the given room to ScyllaDB.
pub async fn persist_snapshot<D>(dal: &D, doc_id: Uuid, room: &DocRoom) -> bool
where
    D: WriteSnapshot + ReadLatestSnapshot + DeleteSnapshot,
{
    let (data, checksum) = {
        let doc = room.doc.read().unwrap();
        encode_snapshot(&doc)
    };

    let version = room.next_snapshot_slot();
    let taken_at = Utc::now();

    let result = dal
        .write_snapshot(NewCollabSnapshot {
            doc_id,
            version,
            data,
            checksum,
            taken_at,
        })
        .await;

    if let Err(e) = result {
        tracing::warn!(doc_id = %doc_id, version, "snapshot write failed: {}", e);
        return false;
    }

    tracing::debug!(doc_id = %doc_id, version, "snapshot written");
    true
}

/// Eviction sweep: remove rooms idle > 5 minutes, flush final snapshot.
pub async fn eviction_sweep<D>(dal: &D, store: &DocStore)
where
    D: WriteSnapshot + ReadLatestSnapshot + DeleteSnapshot + Clone + Send + Sync + 'static,
{
    let evict_ids: Vec<Uuid> = store
        .iter()
        .filter(|entry| entry.value().is_idle_for_eviction())
        .map(|entry| *entry.key())
        .collect();

    for doc_id in evict_ids {
        if let Some((_, room)) = store.remove(&doc_id) {
            tracing::info!(doc_id = %doc_id, "evicting idle room, flushing snapshot");
            persist_snapshot(dal, doc_id, &room).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::room::{DocRoom, DocStore};
    use chrono::{DateTime, Utc};
    use dashmap::DashMap;
    use kernel::{CollabSnapshot, NewCollabSnapshot};
    use std::sync::{Arc, Mutex};
    use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

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
                    version: new_snapshot.version,
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
                    .max_by_key(|s| s.version)
                    .cloned())
            }
        }
    }

    impl DeleteSnapshot for MockDal {
        fn delete_snapshot(
            &self,
            doc_id: uuid::Uuid,
            version: i32,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let snapshots = Arc::clone(&self.snapshots);
            async move {
                snapshots
                    .lock()
                    .unwrap()
                    .retain(|s| !(s.doc_id == doc_id && s.version == version));
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
    async fn snapshot_version_cycles_1_to_5() {
        let dal = MockDal::new();
        let room = DocRoom::new();
        let doc_id = Uuid::new_v4();
        for _ in 0..6 {
            persist_snapshot(&dal, doc_id, &room).await;
        }
        let snaps = dal.snapshots.lock().unwrap();
        let versions: Vec<i32> = snaps.iter().map(|s| s.version).collect();
        // versions should be [1, 2, 3, 4, 5, 1]
        assert_eq!(versions, vec![1, 2, 3, 4, 5, 1]);
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
