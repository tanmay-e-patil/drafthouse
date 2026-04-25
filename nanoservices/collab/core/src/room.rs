use bytes::Bytes;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Instant;
use tokio::sync::broadcast;
use uuid::Uuid;
use yrs::Doc;

pub const MAX_EDITORS: usize = 100;
pub const MAX_DOC_BYTES: usize = 1_048_576; // 1 MB
pub const MAX_MSG_BYTES: usize = 102_400; // 100 KB
pub const SNAPSHOT_OPS_THRESHOLD: usize = 100;
pub const SNAPSHOT_INTERVAL_SECS: u64 = 30;
pub const EVICTION_IDLE_SECS: u64 = 300; // 5 min
pub const EVICTION_SWEEP_SECS: u64 = 60;
pub const SNAPSHOT_RING_SIZE: i32 = 5;

const BROADCAST_CAPACITY: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwarenessPeer {
    pub name: String,
    pub color: String,
    pub last_active_ms: i64,
}

pub struct DocRoom {
    pub doc: Arc<std::sync::RwLock<Doc>>,
    pub connections: AtomicUsize,
    pub op_count: AtomicUsize,
    pub last_empty_at: Mutex<Option<Instant>>,
    pub last_snapshot_at: Mutex<Instant>,
    pub next_snapshot_version: Mutex<i32>,
    pub awareness: DashMap<u64, AwarenessPeer>,
    pub connection_awareness: DashMap<u64, Vec<u64>>,
    /// Broadcast channel: all WS sessions in this room subscribe.
    pub tx: broadcast::Sender<Bytes>,
}

impl Default for DocRoom {
    fn default() -> Self {
        Self::new()
    }
}

impl DocRoom {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            doc: Arc::new(std::sync::RwLock::new(Doc::new())),
            connections: AtomicUsize::new(0),
            op_count: AtomicUsize::new(0),
            last_empty_at: Mutex::new(Some(Instant::now())),
            last_snapshot_at: Mutex::new(Instant::now()),
            next_snapshot_version: Mutex::new(1),
            awareness: DashMap::new(),
            connection_awareness: DashMap::new(),
            tx,
        }
    }

    pub fn connection_count(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    /// Returns true if the connection was accepted (under the 100-editor cap).
    pub fn add_connection(&self) -> bool {
        let prev = self
            .connections
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                if n < MAX_EDITORS { Some(n + 1) } else { None }
            });
        if prev.is_ok() {
            *self.last_empty_at.lock().unwrap() = None;
            true
        } else {
            tracing::warn!(
                doc_id_hint = "unknown",
                "connection rejected: editor cap reached"
            );
            false
        }
    }

    pub fn remove_connection(&self) {
        let prev = self.connections.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 {
            // just became empty
            *self.last_empty_at.lock().unwrap() = Some(Instant::now());
        }
    }

    /// Increment op counter, return new count.
    pub fn increment_ops(&self) -> usize {
        self.op_count.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// True if a snapshot should be triggered (100 ops or 30s elapsed).
    pub fn should_snapshot(&self) -> bool {
        let op_count = self.op_count.load(Ordering::SeqCst);
        if op_count > 0 && op_count.is_multiple_of(SNAPSHOT_OPS_THRESHOLD) {
            return true;
        }
        let elapsed = self.last_snapshot_at.lock().unwrap().elapsed().as_secs();
        elapsed >= SNAPSHOT_INTERVAL_SECS
    }

    /// Advance to next ring-buffer slot (1–5) and reset counters.
    pub fn next_snapshot_slot(&self) -> i32 {
        let mut v = self.next_snapshot_version.lock().unwrap();
        let slot = *v;
        *v = if slot >= SNAPSHOT_RING_SIZE {
            1
        } else {
            slot + 1
        };
        *self.last_snapshot_at.lock().unwrap() = Instant::now();
        slot
    }

    pub fn is_idle_for_eviction(&self) -> bool {
        if let Some(t) = *self.last_empty_at.lock().unwrap() {
            t.elapsed().as_secs() >= EVICTION_IDLE_SECS
        } else {
            false
        }
    }

    pub fn upsert_awareness(&self, client_id: u64, peer: AwarenessPeer) {
        self.awareness.insert(client_id, peer);
    }

    pub fn remove_awareness(&self, client_id: u64) {
        self.awareness.remove(&client_id);
    }

    pub fn awareness_peers(&self) -> Vec<AwarenessPeer> {
        self.awareness
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub fn apply_awareness_update(
        &self,
        connection_id: u64,
        updates: Vec<(u64, Option<AwarenessPeer>)>,
    ) {
        let mut tracked = self
            .connection_awareness
            .get(&connection_id)
            .map(|entry| entry.value().clone())
            .unwrap_or_default();

        for (client_id, peer) in updates {
            if !tracked.contains(&client_id) {
                tracked.push(client_id);
            }

            match peer {
                Some(peer) => {
                    self.awareness.insert(client_id, peer);
                }
                None => {
                    self.awareness.remove(&client_id);
                }
            }
        }

        self.connection_awareness.insert(connection_id, tracked);
    }

    pub fn remove_connection_awareness(&self, connection_id: u64) {
        if let Some((_, client_ids)) = self.connection_awareness.remove(&connection_id) {
            for client_id in client_ids {
                self.awareness.remove(&client_id);
            }
        }
    }
}

pub fn awareness_last_active_to_datetime(last_active_ms: i64) -> Option<DateTime<Utc>> {
    DateTime::<Utc>::from_timestamp_millis(last_active_ms)
}

pub type DocStore = DashMap<Uuid, Arc<DocRoom>>;

pub fn get_or_create_room(store: &DocStore, doc_id: Uuid) -> Arc<DocRoom> {
    store
        .entry(doc_id)
        .or_insert_with(|| {
            tracing::debug!(doc_id = %doc_id, "room created");
            Arc::new(DocRoom::new())
        })
        .clone()
}

/// Encode the current doc state as a snapshot blob + SHA256 checksum.
pub fn encode_snapshot(doc: &Doc) -> (Vec<u8>, String) {
    use sha2::{Digest, Sha256};
    use yrs::{ReadTxn, StateVector, Transact};

    let txn = doc.transact();
    let data = txn.encode_state_as_update_v1(&StateVector::default());
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let checksum = hex::encode(hasher.finalize());
    (data, checksum)
}

/// Verify a loaded snapshot's checksum.
pub fn verify_snapshot_checksum(data: &[u8], expected: &str) -> bool {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize()) == expected
}

#[cfg(test)]
mod tests {
    use super::*;
    use yrs::{Text, Transact};

    fn make_room() -> DocRoom {
        DocRoom::new()
    }

    #[test]
    fn add_connection_increments_counter() {
        let room = make_room();
        assert!(room.add_connection());
        assert_eq!(room.connection_count(), 1);
    }

    #[test]
    fn remove_connection_decrements_counter() {
        let room = make_room();
        room.add_connection();
        room.remove_connection();
        assert_eq!(room.connection_count(), 0);
    }

    #[test]
    fn room_starts_with_last_empty_at_set() {
        let room = make_room();
        assert!(room.last_empty_at.lock().unwrap().is_some());
    }

    #[test]
    fn add_connection_clears_last_empty_at() {
        let room = make_room();
        room.add_connection();
        assert!(room.last_empty_at.lock().unwrap().is_none());
    }

    #[test]
    fn remove_last_connection_sets_last_empty_at() {
        let room = make_room();
        room.add_connection();
        room.remove_connection();
        assert!(room.last_empty_at.lock().unwrap().is_some());
    }

    #[test]
    fn cap_at_100_editors() {
        let room = make_room();
        for _ in 0..MAX_EDITORS {
            assert!(room.add_connection());
        }
        // 101st is rejected
        assert!(!room.add_connection());
        assert_eq!(room.connection_count(), MAX_EDITORS);
    }

    #[test]
    fn should_snapshot_after_100_ops() {
        let room = make_room();
        for _ in 0..99 {
            room.increment_ops();
        }
        assert!(!room.should_snapshot());
        room.increment_ops(); // 100th op
        assert!(room.should_snapshot());
    }

    #[test]
    fn snapshot_slot_cycles_1_to_5() {
        let room = make_room();
        let slots: Vec<i32> = (0..7).map(|_| room.next_snapshot_slot()).collect();
        assert_eq!(&slots[..5], &[1, 2, 3, 4, 5]);
        assert_eq!(slots[5], 1); // wraps back to 1
        assert_eq!(slots[6], 2);
    }

    #[test]
    fn encode_snapshot_checksum_verifies() {
        let doc = Doc::new();
        {
            let text = doc.get_or_insert_text("content");
            let mut txn = doc.transact_mut();
            text.insert(&mut txn, 0, "test content");
        }
        let (data, checksum) = encode_snapshot(&doc);
        assert!(verify_snapshot_checksum(&data, &checksum));
        assert!(!verify_snapshot_checksum(&data, "badhash"));
    }

    #[test]
    fn get_or_create_room_returns_same_room_for_same_id() {
        let store: DocStore = DashMap::new();
        let id = Uuid::new_v4();
        let r1 = get_or_create_room(&store, id);
        let r2 = get_or_create_room(&store, id);
        assert!(Arc::ptr_eq(&r1, &r2));
    }

    #[test]
    fn get_or_create_room_returns_different_rooms_for_different_ids() {
        let store: DocStore = DashMap::new();
        let r1 = get_or_create_room(&store, Uuid::new_v4());
        let r2 = get_or_create_room(&store, Uuid::new_v4());
        assert!(!Arc::ptr_eq(&r1, &r2));
    }

    #[test]
    fn awareness_peers_can_be_added_and_removed() {
        let room = make_room();
        room.upsert_awareness(
            7,
            AwarenessPeer {
                name: "alice".into(),
                color: "#E53E3E".into(),
                last_active_ms: 1_700_000_000_000,
            },
        );

        let peers = room.awareness_peers();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].name, "alice");

        room.remove_awareness(7);
        assert!(room.awareness_peers().is_empty());
    }

    #[test]
    fn awareness_last_active_converts_to_datetime() {
        let dt = awareness_last_active_to_datetime(1_700_000_000_000).unwrap();
        assert_eq!(dt.timestamp_millis(), 1_700_000_000_000);
    }

    #[test]
    fn apply_awareness_update_tracks_and_removes_connection_clients() {
        let room = make_room();
        room.apply_awareness_update(
            11,
            vec![(
                7,
                Some(AwarenessPeer {
                    name: "alice".into(),
                    color: "#E53E3E".into(),
                    last_active_ms: 1_700_000_000_000,
                }),
            )],
        );

        assert_eq!(room.awareness_peers().len(), 1);

        room.remove_connection_awareness(11);
        assert!(room.awareness_peers().is_empty());
    }
}
