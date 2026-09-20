use dal::PurgeCollabData;
use kernel::{DocumentAccessChange, DocumentAccessChanged};
use nan_serve_event_subscriber::subscribe_to_event;
use std::time::Duration;
use tracing::{error, warn};
use uuid::Uuid;

use crate::collab_dal_ref::get_collab_dal;
use crate::doc_store_ref::get_doc_store;
use crate::room::{DocStore, RoomAccessChange};

const PURGE_ATTEMPTS: usize = 3;
const PURGE_RETRY_DELAY: Duration = Duration::from_millis(100);

#[subscribe_to_event]
async fn on_document_access_changed(event: DocumentAccessChanged) {
    if let DocumentAccessChange::Deleted = event.change {
        // Erasure must not depend on whether any room is cached: disconnect
        // and remove what exists, then purge durable storage regardless.
        let store = get_doc_store().map(|store| store.as_ref());
        teardown_deleted_document(store, event.doc_id).await;
        return;
    }
    let Some(store) = get_doc_store() else {
        warn!("on_document_access_changed: DocStore not initialised");
        return;
    };
    apply_document_access_change(store.as_ref(), event).await;
}

pub(crate) async fn apply_document_access_change(store: &DocStore, event: DocumentAccessChanged) {
    match event.change {
        DocumentAccessChange::Deleted => teardown_deleted_document(Some(store), event.doc_id).await,
        DocumentAccessChange::Visibility { is_public: false } => {
            if let Some(room) = store.get(&event.doc_id) {
                room.notify_access_change(RoomAccessChange::DisconnectAnonymous);
            }
        }
        DocumentAccessChange::Visibility { is_public: true } => {}
        DocumentAccessChange::Member { user_id, .. } => {
            if let Some(room) = store.get(&event.doc_id) {
                room.notify_access_change(RoomAccessChange::DisconnectUser(user_id));
            }
        }
    }
}

/// Right-to-erasure (#7): disconnect every session, remove the room so no new
/// session or snapshot can target it, permanently stop durable writes, then
/// purge Scylla WAL/snapshots with bounded retries.
async fn teardown_deleted_document(store: Option<&DocStore>, doc_id: Uuid) {
    let room = store.and_then(|store| store.remove(&doc_id).map(|(_, room)| room));
    if let Some(room) = room {
        room.notify_access_change(RoomAccessChange::DisconnectAll);
        room.serialize_update(|| async {
            room.close_for_writes();
            purge_deleted_document(doc_id).await;
        })
        .await;
    } else {
        purge_deleted_document(doc_id).await;
    }
}

async fn purge_deleted_document(doc_id: Uuid) {
    let Some(dal) = get_collab_dal() else {
        warn!(doc_id = %doc_id, "collab DAL not initialised; Scylla purge skipped");
        return;
    };
    if let Err(last_error) = purge_collab_data_with_retries(dal.as_ref(), doc_id).await {
        error!(
            doc_id = %doc_id,
            %last_error,
            attempts = PURGE_ATTEMPTS,
            "Scylla purge failed after retries; erasure incomplete"
        );
    }
}

pub(crate) async fn purge_collab_data_with_retries<D>(
    dal: &D,
    doc_id: Uuid,
) -> Result<(), utils::errors::NanoServiceError>
where
    D: PurgeCollabData,
{
    let mut last_error = None;
    for attempt in 1..=PURGE_ATTEMPTS {
        match dal.purge_collab_data(doc_id).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                warn!(doc_id = %doc_id, attempt, %e, "Scylla purge attempt failed");
                last_error = Some(e);
                if attempt < PURGE_ATTEMPTS {
                    tokio::time::sleep(PURGE_RETRY_DELAY).await;
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| {
        utils::errors::NanoServiceError::new(
            "Scylla purge exhausted retries",
            utils::errors::NanoServiceErrorStatus::InternalServerError,
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::room::{DocRoom, RoomAccessChange};
    use dashmap::DashMap;
    use std::sync::{Arc, Mutex};
    use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

    async fn apply(store: &DocStore, event: DocumentAccessChanged) {
        apply_document_access_change(store, event).await;
    }

    #[tokio::test]
    async fn access_events_signal_only_the_affected_sessions() {
        let store = DashMap::new();
        let doc_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let room = Arc::new(DocRoom::new());
        store.insert(doc_id, room.clone());
        let mut changes = room.subscribe_to_access_changes();

        apply(
            &store,
            DocumentAccessChanged {
                doc_id,
                change: DocumentAccessChange::Visibility { is_public: false },
            },
        )
        .await;
        assert_eq!(
            changes.recv().await.unwrap(),
            RoomAccessChange::DisconnectAnonymous
        );

        apply(
            &store,
            DocumentAccessChanged {
                doc_id,
                change: DocumentAccessChange::Member {
                    user_id,
                    role: None,
                },
            },
        )
        .await;
        assert_eq!(
            changes.recv().await.unwrap(),
            RoomAccessChange::DisconnectUser(user_id)
        );

        apply(
            &store,
            DocumentAccessChanged {
                doc_id,
                change: DocumentAccessChange::Deleted,
            },
        )
        .await;
        assert_eq!(
            changes.recv().await.unwrap(),
            RoomAccessChange::DisconnectAll
        );
    }

    #[tokio::test]
    async fn making_a_document_public_does_not_disconnect_sessions() {
        let store = DashMap::new();
        let doc_id = Uuid::new_v4();
        let room = Arc::new(DocRoom::new());
        store.insert(doc_id, room.clone());
        let mut changes = room.subscribe_to_access_changes();

        apply(
            &store,
            DocumentAccessChanged {
                doc_id,
                change: DocumentAccessChange::Visibility { is_public: true },
            },
        )
        .await;

        if changes.try_recv().is_ok() {
            panic!("making a document public must not disconnect sessions");
        }
    }

    #[tokio::test]
    async fn deletion_removes_and_closes_the_room() {
        let store = DashMap::new();
        let doc_id = Uuid::new_v4();
        let room = Arc::new(DocRoom::new());
        store.insert(doc_id, room.clone());
        let mut changes = room.subscribe_to_access_changes();

        apply(
            &store,
            DocumentAccessChanged {
                doc_id,
                change: DocumentAccessChange::Deleted,
            },
        )
        .await;

        assert!(!store.contains_key(&doc_id), "room must be removed");
        assert!(room.is_closed(), "room must accept no further writes");
        assert_eq!(
            changes.recv().await.unwrap(),
            RoomAccessChange::DisconnectAll
        );
    }

    #[derive(Clone, Default)]
    struct FlakyPurge {
        attempts: Arc<Mutex<Vec<Uuid>>>,
        fail_first: usize,
    }

    impl PurgeCollabData for FlakyPurge {
        async fn purge_collab_data(&self, doc_id: Uuid) -> Result<(), NanoServiceError> {
            let mut attempts = self.attempts.lock().unwrap();
            attempts.push(doc_id);
            if attempts.len() <= self.fail_first {
                return Err(NanoServiceError::new(
                    "injected purge failure",
                    NanoServiceErrorStatus::InternalServerError,
                ));
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn purge_retries_transient_failures_until_it_succeeds() {
        let dal = FlakyPurge {
            fail_first: 2,
            ..Default::default()
        };
        let doc_id = Uuid::new_v4();

        purge_collab_data_with_retries(&dal, doc_id).await.unwrap();

        assert_eq!(dal.attempts.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn purge_gives_up_after_the_configured_attempts() {
        let dal = FlakyPurge {
            fail_first: usize::MAX,
            ..Default::default()
        };
        let doc_id = Uuid::new_v4();

        assert!(purge_collab_data_with_retries(&dal, doc_id).await.is_err());
        assert_eq!(dal.attempts.lock().unwrap().len(), PURGE_ATTEMPTS);
    }
}
