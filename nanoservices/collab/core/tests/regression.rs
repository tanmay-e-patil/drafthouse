//! Regression tests for the collaboration-core audit (docs/BUG_AUDIT.md).
//!
//! Each `regression_NN_...` test asserts the REQUIRED behavior for audit
//! finding #NN. They are intentionally red while the bug is unfixed; a fix
//! must turn its test green without weakening the assertion.

use collab_core::{room::*, snapshot::*, sync_protocol::*};
use dal::{DeleteSnapshot, ReadLatestSnapshot, WriteSnapshot};
use kernel::{CollabSnapshot, NewCollabSnapshot};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};
use uuid::Uuid;
use yrs::{Doc, GetString, ReadTxn, StateVector, Text, Transact};

#[derive(Clone, Default)]
struct Storage {
    saved: Arc<Mutex<Vec<NewCollabSnapshot>>>,
    fail: bool,
    activate_on_write: Option<Arc<DocRoom>>,
}
impl WriteSnapshot for Storage {
    async fn write_snapshot(&self, snapshot: NewCollabSnapshot) -> Result<(), NanoServiceError> {
        if let Some(room) = &self.activate_on_write {
            room.add_connection();
        }
        if self.fail {
            return Err(NanoServiceError::new(
                "injected outage",
                NanoServiceErrorStatus::InternalServerError,
            ));
        }
        self.saved.lock().unwrap().push(snapshot);
        Ok(())
    }
}
impl ReadLatestSnapshot for Storage {
    async fn read_latest_snapshot(
        &self,
        _: Uuid,
    ) -> Result<Option<CollabSnapshot>, NanoServiceError> {
        panic!("unexpected read: current production never restores a snapshot")
    }
}
impl DeleteSnapshot for Storage {
    async fn delete_snapshot(&self, _: Uuid, _: i32) -> Result<(), NanoServiceError> {
        Ok(())
    }
}
fn text(room: &DocRoom) -> String {
    let doc = room.doc.read().unwrap();
    doc.get_or_insert_text("content")
        .get_string(&doc.transact())
}
fn seed(room: &DocRoom, content: &str) {
    let doc = room.doc.read().unwrap();
    doc.get_or_insert_text("content")
        .insert(&mut doc.transact_mut(), 0, content);
}
fn idle(room: &DocRoom) {
    *room.last_empty_at.lock().unwrap() = Some(Instant::now() - Duration::from_secs(301));
}

/// REG-19 (#19): a relayed awareness update must not reassign the peer's
/// authenticated identity, and a disconnecting connection must remove only
/// the clients it owns.
#[test]
fn regression_19_relay_keeps_identity_and_disconnect_removes_only_owned_clients() {
    let room = DocRoom::new();
    let alice = Uuid::new_v4();
    let bob = Uuid::new_v4();
    let peer = |user| AwarenessPeer {
        user_id: Some(user),
        name: "alice".into(),
        color: "red".into(),
        last_active_ms: 1,
    };
    // Alice's connection introduces client 7.
    room.apply_awareness_update(1, vec![(7, Some(peer(alice)))]);
    // Bob's connection relays client 7 (normal y-websocket retransmission).
    room.apply_awareness_update(2, vec![(7, Some(peer(bob)))]);
    assert_eq!(room.awareness_peers()[0].user_id, Some(alice));
    // Bob leaving must not delete Alice's presence.
    room.remove_connection_awareness(2);
    assert_eq!(room.awareness_peers().len(), 1);
    assert_eq!(room.awareness_peers()[0].user_id, Some(alice));
    room.remove_connection_awareness(1);
    assert!(room.awareness_peers().is_empty());
}

/// REG-24 (#24): a malformed protocol length must be rejected, never panic.
#[test]
fn regression_24_malformed_length_is_rejected_without_panicking() {
    let mut data = vec![0, 2];
    data.extend_from_slice(&[255, 255, 255, 255, 255, 255, 255, 255, 255, 1]);
    assert!(matches!(decode_message(&data), CollabMessage::Unknown));
}

/// REG-26 (#26): a failed snapshot flush must not discard the live room —
/// the in-memory state is the only recovery source until persistence succeeds.
#[tokio::test]
async fn regression_26_failed_snapshot_retains_room() {
    let store = DocStore::new();
    let id = Uuid::new_v4();
    let room = get_or_create_room(&store, id);
    seed(&room, "not saved");
    idle(&room);
    eviction_sweep(
        &Storage {
            fail: true,
            ..Storage::default()
        },
        &store,
    )
    .await;
    assert!(store.contains_key(&id));
}

/// REG-26 (#26): a connection arriving while the sweep is persisting must
/// keep the room registered; later lookups return the same room.
#[tokio::test]
async fn regression_26_connection_arriving_during_sweep_keeps_room() {
    let store = DocStore::new();
    for _ in 0..2 {
        let room = get_or_create_room(&store, Uuid::new_v4());
        idle(&room);
    }
    let ids: Vec<_> = store.iter().map(|e| *e.key()).collect();
    let victim = store.get(&ids[1]).unwrap().clone();
    let storage = Storage {
        activate_on_write: Some(victim.clone()),
        ..Storage::default()
    };
    eviction_sweep(&storage, &store).await;
    assert!(victim.connection_count() > 0);
    assert!(store.contains_key(&ids[1]));
    assert!(Arc::ptr_eq(&victim, &get_or_create_room(&store, ids[1])));
}

/// REG-27 (#27): the background sweep must flush rooms whose snapshot is due,
/// even while editors are connected.
#[tokio::test]
async fn regression_27_due_room_is_snapshotted_by_sweep() {
    let store = DocStore::new();
    let room = get_or_create_room(&store, Uuid::new_v4());
    room.add_connection();
    seed(&room, "dirty");
    room.increment_ops();
    *room.last_snapshot_at.lock().unwrap() = Instant::now() - Duration::from_secs(31);
    assert!(room.should_snapshot());
    let storage = Storage::default();
    eviction_sweep(&storage, &store).await;
    assert!(!storage.saved.lock().unwrap().is_empty());
}
