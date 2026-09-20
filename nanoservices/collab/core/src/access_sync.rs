use kernel::{DocumentAccessChange, DocumentAccessChanged};
use nan_serve_event_subscriber::subscribe_to_event;
use tracing::warn;

use crate::doc_store_ref::get_doc_store;
use crate::room::RoomAccessChange;

#[subscribe_to_event]
async fn on_document_access_changed(event: DocumentAccessChanged) {
    let Some(store) = get_doc_store() else {
        warn!("on_document_access_changed: DocStore not initialised");
        return;
    };
    apply_document_access_change(store, event);
}

pub(crate) fn apply_document_access_change(store: &crate::DocStore, event: DocumentAccessChanged) {
    let Some(room) = store.get(&event.doc_id) else {
        return;
    };

    let change = match event.change {
        DocumentAccessChange::Deleted => RoomAccessChange::DisconnectAll,
        DocumentAccessChange::Visibility { is_public: false } => {
            RoomAccessChange::DisconnectAnonymous
        }
        DocumentAccessChange::Visibility { is_public: true } => return,
        DocumentAccessChange::Member { user_id, .. } => RoomAccessChange::DisconnectUser(user_id),
    };
    room.notify_access_change(change);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::room::{DocRoom, RoomAccessChange};
    use dashmap::DashMap;
    use std::sync::Arc;
    use uuid::Uuid;

    #[tokio::test]
    async fn access_events_signal_only_the_affected_sessions() {
        let store = DashMap::new();
        let doc_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let room = Arc::new(DocRoom::new());
        store.insert(doc_id, room.clone());
        let mut changes = room.subscribe_to_access_changes();

        apply_document_access_change(
            &store,
            DocumentAccessChanged {
                doc_id,
                change: DocumentAccessChange::Visibility { is_public: false },
            },
        );
        assert_eq!(
            changes.recv().await.unwrap(),
            RoomAccessChange::DisconnectAnonymous
        );

        apply_document_access_change(
            &store,
            DocumentAccessChanged {
                doc_id,
                change: DocumentAccessChange::Member {
                    user_id,
                    role: None,
                },
            },
        );
        assert_eq!(
            changes.recv().await.unwrap(),
            RoomAccessChange::DisconnectUser(user_id)
        );

        apply_document_access_change(
            &store,
            DocumentAccessChanged {
                doc_id,
                change: DocumentAccessChange::Deleted,
            },
        );
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

        apply_document_access_change(
            &store,
            DocumentAccessChanged {
                doc_id,
                change: DocumentAccessChange::Visibility { is_public: true },
            },
        );

        assert!(matches!(changes.try_recv(), Err(_)));
    }
}
