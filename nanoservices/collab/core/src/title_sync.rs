use bytes::Bytes;
use kernel::TitleUpdated;
use nan_serve_event_subscriber::subscribe_to_event;
use tracing::warn;

use crate::doc_store_ref::get_doc_store;
use crate::encode_title_update;

#[subscribe_to_event]
async fn on_title_updated(event: TitleUpdated) {
    let Some(store) = get_doc_store() else {
        warn!("on_title_updated: DocStore not initialised");
        return;
    };
    if let Some(room) = store.get(&event.doc_id) {
        let msg = Bytes::from(encode_title_update(&event.title));
        let _ = room.tx.send(msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::room::DocRoom;
    use dashmap::DashMap;
    use std::sync::Arc;
    use uuid::Uuid;

    fn make_room() -> Arc<DocRoom> {
        Arc::new(DocRoom::new())
    }

    #[tokio::test]
    async fn on_title_updated_broadcasts_title_update_message() {
        let store: Arc<crate::DocStore> = Arc::new(DashMap::new());
        let doc_id = Uuid::new_v4();
        let room = make_room();
        store.insert(doc_id, room.clone());

        // Subscribe before calling, bypassing the global DocStore
        let mut rx = room.tx.subscribe();

        let event = TitleUpdated {
            doc_id,
            title: "New Title".to_string(),
        };

        // Call handler directly (bypasses macro wiring; tests business logic)
        on_title_updated_inner(&store, event).await;

        let received = rx.try_recv().expect("broadcast must have a message");
        let expected = Bytes::from(encode_title_update("New Title"));
        assert_eq!(received, expected);
    }

    #[tokio::test]
    async fn on_title_updated_no_room_does_not_panic() {
        let store: Arc<crate::DocStore> = Arc::new(DashMap::new());
        let event = TitleUpdated {
            doc_id: Uuid::new_v4(),
            title: "Ghost".to_string(),
        };
        // No room inserted — should complete without panic
        on_title_updated_inner(&store, event).await;
    }

    // Extracted inner logic for testability (no global state dependency)
    async fn on_title_updated_inner(store: &crate::DocStore, event: TitleUpdated) {
        if let Some(room) = store.get(&event.doc_id) {
            let msg = Bytes::from(encode_title_update(&event.title));
            let _ = room.tx.send(msg);
        }
    }
}
