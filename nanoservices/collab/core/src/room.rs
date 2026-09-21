use bytes::Bytes;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::time::Instant;
use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    future::Future,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering},
    },
};
use tokio::sync::{Mutex as AsyncMutex, OnceCell, broadcast};
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
    pub user_id: Option<Uuid>,
    pub name: String,
    pub color: String,
    pub last_active_ms: i64,
}

struct StoredAwareness {
    owner_connection_id: Option<u64>,
    clock: u32,
    // The outer option is absent until a non-null state establishes identity;
    // the inner option distinguishes an anonymous owner from an authenticated one.
    user_id: Option<Option<Uuid>>,
    peer: Option<AwarenessPeer>,
}

#[derive(Default)]
struct RoomAwareness {
    clients: HashMap<u64, StoredAwareness>,
    clients_by_connection: HashMap<u64, HashSet<u64>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomAccessChange {
    DisconnectAll,
    DisconnectAnonymous,
    DisconnectUser(Uuid),
}

impl RoomAccessChange {
    pub fn disconnects(self, user_id: Option<Uuid>) -> bool {
        match self {
            Self::DisconnectAll => true,
            Self::DisconnectAnonymous => user_id.is_none(),
            Self::DisconnectUser(target) => user_id == Some(target),
        }
    }
}

pub struct DocRoom {
    pub doc: Arc<std::sync::RwLock<Doc>>,
    pub connections: AtomicUsize,
    pub op_count: AtomicUsize,
    pub last_empty_at: Mutex<Option<Instant>>,
    pub last_snapshot_at: Mutex<Instant>,
    sequence: AtomicI64,
    next_snapshot_generation: Mutex<i64>,
    awareness: Mutex<RoomAwareness>,
    initialization: OnceCell<()>,
    update_gate: AsyncMutex<()>,
    /// Set when the document is deleted; the room must accept no further
    /// durable writes so purged storage cannot be repopulated.
    closed: AtomicBool,
    /// Broadcast channel: all WS sessions in this room subscribe.
    pub tx: broadcast::Sender<Bytes>,
    /// Authorization changes are separate from protocol payloads so sessions
    /// can selectively close without exposing control messages to clients.
    access_tx: broadcast::Sender<RoomAccessChange>,
}

impl Default for DocRoom {
    fn default() -> Self {
        Self::new()
    }
}

impl DocRoom {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        let (access_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            doc: Arc::new(std::sync::RwLock::new(Doc::new())),
            connections: AtomicUsize::new(0),
            op_count: AtomicUsize::new(0),
            last_empty_at: Mutex::new(Some(Instant::now())),
            last_snapshot_at: Mutex::new(Instant::now()),
            sequence: AtomicI64::new(0),
            next_snapshot_generation: Mutex::new(1),
            awareness: Mutex::new(RoomAwareness::default()),
            initialization: OnceCell::new(),
            update_gate: AsyncMutex::new(()),
            closed: AtomicBool::new(false),
            tx,
            access_tx,
        }
    }

    pub fn subscribe_to_access_changes(&self) -> broadcast::Receiver<RoomAccessChange> {
        self.access_tx.subscribe()
    }

    pub fn notify_access_change(&self, change: RoomAccessChange) {
        let _ = self.access_tx.send(change);
    }

    pub fn connection_count(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    /// Permanently stop accepting durable writes for this room.
    pub fn close_for_writes(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    pub async fn ensure_initialized<E, F, Fut>(&self, initialize: F) -> Result<(), E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(), E>>,
    {
        self.initialization
            .get_or_try_init(initialize)
            .await
            .map(|_| ())
    }

    pub async fn serialize_update<T, F, Fut>(&self, update: F) -> T
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        let _guard = self.update_gate.lock().await;
        update().await
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

    /// True if dirty state should be snapshotted (100 ops or 30s elapsed).
    pub fn should_snapshot(&self) -> bool {
        let op_count = self.op_count.load(Ordering::SeqCst);
        if op_count == 0 {
            return false;
        }
        if op_count >= SNAPSHOT_OPS_THRESHOLD {
            return true;
        }
        let elapsed = self.last_snapshot_at.lock().unwrap().elapsed().as_secs();
        elapsed >= SNAPSHOT_INTERVAL_SECS
    }

    pub fn mark_snapshot_persisted(&self) {
        self.op_count.store(0, Ordering::SeqCst);
        *self.last_snapshot_at.lock().unwrap() = Instant::now();
    }

    pub fn next_operation_sequence(&self) -> i64 {
        self.sequence.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn current_sequence(&self) -> i64 {
        self.sequence.load(Ordering::SeqCst)
    }

    pub fn restore_progress(&self, sequence: i64, generation: i64) {
        self.sequence.fetch_max(sequence, Ordering::SeqCst);
        let mut next_generation = self.next_snapshot_generation.lock().unwrap();
        *next_generation = (*next_generation).max(generation + 1);
    }

    /// Allocate a monotonically increasing snapshot generation.
    pub fn next_snapshot_generation(&self) -> i64 {
        let mut generation = self.next_snapshot_generation.lock().unwrap();
        let current = *generation;
        *generation += 1;
        current
    }

    pub fn is_idle_for_eviction(&self) -> bool {
        if let Some(t) = *self.last_empty_at.lock().unwrap() {
            t.elapsed().as_secs() >= EVICTION_IDLE_SECS
        } else {
            false
        }
    }

    pub fn upsert_awareness(&self, client_id: u64, peer: AwarenessPeer) {
        self.awareness
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clients
            .insert(
                client_id,
                StoredAwareness {
                    owner_connection_id: None,
                    clock: 0,
                    user_id: Some(peer.user_id),
                    peer: Some(peer),
                },
            );
    }

    pub fn remove_awareness(&self, client_id: u64) {
        self.awareness
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clients
            .remove(&client_id);
    }

    pub fn awareness_peers(&self) -> Vec<AwarenessPeer> {
        self.awareness
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clients
            .values()
            .filter_map(|state| state.peer.clone())
            .collect()
    }

    pub fn presence_peers(&self) -> Vec<AwarenessPeer> {
        let mut anonymous = Vec::new();
        let mut by_user_id: HashMap<Uuid, AwarenessPeer> = HashMap::new();

        for peer in self.awareness_peers() {
            let Some(user_id) = peer.user_id else {
                anonymous.push(peer);
                continue;
            };

            match by_user_id.get(&user_id) {
                Some(existing) if existing.last_active_ms >= peer.last_active_ms => {}
                _ => {
                    by_user_id.insert(user_id, peer);
                }
            }
        }

        anonymous.extend(by_user_id.into_values());
        anonymous
    }

    pub fn apply_awareness_update(
        &self,
        connection_id: u64,
        updates: Vec<(u64, u32, Option<AwarenessPeer>)>,
    ) {
        let mut awareness = self
            .awareness
            .lock()
            .unwrap_or_else(PoisonError::into_inner);

        for (client_id, clock, peer) in updates {
            match awareness.clients.entry(client_id) {
                Entry::Vacant(entry) => {
                    let user_id = peer.as_ref().map(|peer| peer.user_id);
                    entry.insert(StoredAwareness {
                        owner_connection_id: Some(connection_id),
                        clock,
                        user_id,
                        peer,
                    });
                    awareness
                        .clients_by_connection
                        .entry(connection_id)
                        .or_default()
                        .insert(client_id);
                }
                Entry::Occupied(mut entry) => {
                    let current = entry.get_mut();
                    if current.owner_connection_id != Some(connection_id) {
                        continue;
                    }

                    let removes_current_state =
                        clock == current.clock && peer.is_none() && current.peer.is_some();
                    if clock < current.clock || (clock == current.clock && !removes_current_state) {
                        continue;
                    }

                    current.clock = clock;
                    current.peer = peer.map(|mut peer| {
                        if let Some(user_id) = current.user_id {
                            peer.user_id = user_id;
                        } else {
                            current.user_id = Some(peer.user_id);
                        }
                        peer
                    });
                }
            }
        }
    }

    pub fn remove_connection_awareness(&self, connection_id: u64) {
        let mut awareness = self
            .awareness
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(client_ids) = awareness.clients_by_connection.remove(&connection_id) {
            for client_id in client_ids {
                if awareness
                    .clients
                    .get(&client_id)
                    .is_some_and(|state| state.owner_connection_id == Some(connection_id))
                {
                    awareness.clients.remove(&client_id);
                }
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
    fn snapshot_generations_are_monotonic() {
        let room = make_room();
        let generations: Vec<i64> = (0..7).map(|_| room.next_snapshot_generation()).collect();
        assert_eq!(generations, vec![1, 2, 3, 4, 5, 6, 7]);
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
                user_id: None,
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
                1,
                Some(AwarenessPeer {
                    user_id: None,
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

    #[test]
    fn presence_peers_deduplicates_authenticated_user_by_latest_activity() {
        let room = make_room();
        let user_id = Uuid::new_v4();
        room.upsert_awareness(
            7,
            AwarenessPeer {
                user_id: Some(user_id),
                name: "alice".into(),
                color: "#E53E3E".into(),
                last_active_ms: 1_700_000_000_000,
            },
        );
        room.upsert_awareness(
            8,
            AwarenessPeer {
                user_id: Some(user_id),
                name: "alice".into(),
                color: "#3182CE".into(),
                last_active_ms: 1_700_000_001_000,
            },
        );

        let peers = room.presence_peers();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].user_id, Some(user_id));
        assert_eq!(peers[0].color, "#3182CE");
        assert_eq!(peers[0].last_active_ms, 1_700_000_001_000);
    }

    #[test]
    fn presence_peers_keeps_anonymous_sessions_separate() {
        let room = make_room();
        for client_id in [7, 8] {
            room.upsert_awareness(
                client_id,
                AwarenessPeer {
                    user_id: None,
                    name: "Anonymous".into(),
                    color: "#E53E3E".into(),
                    last_active_ms: 1_700_000_000_000,
                },
            );
        }

        assert_eq!(room.presence_peers().len(), 2);
    }
}
