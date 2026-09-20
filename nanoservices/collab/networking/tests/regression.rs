//! Regression tests for the collaboration networking audit (docs/BUG_AUDIT.md).
//!
//! Runs the unchanged production `handlers.rs` (included verbatim) behind an
//! in-memory DAL, a real Actix WebSocket server and raw TCP frames. Each
//! `regression_NN_...` test asserts the REQUIRED behavior for audit finding
//! #NN and is intentionally red while the bug is unfixed.

mod dal {
    pub use ::dal::*;
    use kernel::{CollabOp, CollabSnapshot, Document, NewCollabOp, NewCollabSnapshot};
    use std::sync::{Arc, Mutex};
    use utils::errors::{NanoServiceError, NanoServiceErrorStatus};
    use uuid::Uuid;
    pub mod postgres_txs {
        use super::*;
        #[derive(Clone)]
        pub struct SqlxPostGresDescriptor(
            pub Arc<Mutex<Option<Document>>>,
            pub Arc<Mutex<Option<String>>>,
            pub Arc<Mutex<i64>>,
            pub bool,
        );
        impl GetDocumentById for SqlxPostGresDescriptor {
            async fn get_document_by_id(
                &self,
                _: Uuid,
            ) -> Result<Option<Document>, NanoServiceError> {
                Ok(self.0.lock().unwrap().clone())
            }
        }
        impl GetDocumentContent for SqlxPostGresDescriptor {
            async fn get_document_content(
                &self,
                _: Uuid,
            ) -> Result<Option<String>, NanoServiceError> {
                Ok(self.1.lock().unwrap().clone())
            }
        }
        impl ProjectDocumentContent for SqlxPostGresDescriptor {
            async fn project_document_content(
                &self,
                _: Uuid,
                content: String,
                revision: i64,
            ) -> Result<bool, NanoServiceError> {
                if self.3 {
                    return Err(NanoServiceError::new(
                        "injected projection failure",
                        NanoServiceErrorStatus::InternalServerError,
                    ));
                }
                let mut current_revision = self.2.lock().unwrap();
                if revision <= *current_revision {
                    return Ok(false);
                }
                *self.1.lock().unwrap() = Some(content);
                *current_revision = revision;
                Ok(true)
            }
        }
    }
    #[derive(Clone, Default)]
    pub struct ScyllaDescriptor {
        pub ops: Arc<Mutex<Vec<NewCollabOp>>>,
        pub snapshots: Arc<Mutex<Vec<NewCollabSnapshot>>>,
        pub fail: bool,
        pub pause: Arc<Mutex<Option<Arc<tokio::sync::Notify>>>>,
        pub entered: Arc<tokio::sync::Notify>,
    }
    impl WriteOp for ScyllaDescriptor {
        async fn write_op(&self, op: NewCollabOp) -> Result<(), NanoServiceError> {
            let pause = self.pause.lock().unwrap().clone();
            if let Some(release) = pause {
                self.entered.notify_one();
                release.notified().await;
            }
            if self.fail {
                return Err(NanoServiceError::new(
                    "injected Scylla outage",
                    NanoServiceErrorStatus::InternalServerError,
                ));
            }
            self.ops.lock().unwrap().push(op);
            Ok(())
        }
    }
    impl WriteSnapshot for ScyllaDescriptor {
        async fn write_snapshot(&self, s: NewCollabSnapshot) -> Result<(), NanoServiceError> {
            self.snapshots.lock().unwrap().push(s);
            Ok(())
        }
    }
    impl ReadLatestSnapshot for ScyllaDescriptor {
        // REG-28 note: "latest" must mean newest by time (or a monotonic
        // generation), never the highest reusable ring slot. The Scylla CQL
        // must match this contract; a disposable-Scylla test should pin it.
        async fn read_latest_snapshot(
            &self,
            doc_id: Uuid,
        ) -> Result<Option<CollabSnapshot>, NanoServiceError> {
            Ok(self
                .snapshots
                .lock()
                .unwrap()
                .iter()
                .filter(|s| s.doc_id == doc_id)
                .max_by_key(|s| s.generation)
                .map(|s| CollabSnapshot {
                    doc_id: s.doc_id,
                    generation: s.generation,
                    through_sequence: s.through_sequence,
                    data: s.data.clone(),
                    checksum: s.checksum.clone(),
                    taken_at: s.taken_at,
                }))
        }
    }
    impl DeleteSnapshot for ScyllaDescriptor {
        async fn delete_snapshot(&self, _: Uuid, _: i64) -> Result<(), NanoServiceError> {
            Ok(())
        }
    }
    impl ReadOpsAfter for ScyllaDescriptor {
        async fn read_ops_after(
            &self,
            doc_id: Uuid,
            sequence: i64,
        ) -> Result<Vec<CollabOp>, NanoServiceError> {
            Ok(self
                .ops
                .lock()
                .unwrap()
                .iter()
                .filter(|o| o.doc_id == doc_id && o.sequence > sequence)
                .map(|o| CollabOp {
                    doc_id: o.doc_id,
                    sequence: o.sequence,
                    created_at: o.created_at,
                    op_id: o.op_id,
                    client_id: o.client_id,
                    data: o.data.clone(),
                })
                .collect())
        }
    }
}
// The handler implementation is included verbatim; no algorithm is reimplemented.
include!("../src/handlers.rs");

mod regression {
    use super::*;
    use actix_web::{App, HttpServer};
    use chrono::Utc;
    use collab_core::{apply_update_safe, room::MAX_DOC_BYTES};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
    };
    use yrs::{Doc, GetString, ReadTxn, StateVector, Text, Transact};

    struct Env {
        port: u16,
        id: Uuid,
        owner: Uuid,
        store: web::Data<DocStore>,
        pg: SqlxPostGresDescriptor,
        storage: ScyllaDescriptor,
        handle: actix_web::dev::ServerHandle,
    }
    impl Env {
        async fn new(fail: bool) -> Self {
            Self::new_with_failures(fail, false).await
        }

        async fn new_with_failures(wal_fail: bool, projection_fail: bool) -> Self {
            let id = Uuid::new_v4();
            let owner = Uuid::new_v4();
            let pg = SqlxPostGresDescriptor(
                Arc::new(Mutex::new(Some(kernel::Document {
                    id,
                    owner_id: owner,
                    title: "regression".into(),
                    is_public: true,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                }))),
                Arc::new(Mutex::new(Some(String::new()))),
                Arc::new(Mutex::new(0)),
                projection_fail,
            );
            let store = web::Data::new(DocStore::new());
            let storage = ScyllaDescriptor {
                fail: wal_fail,
                ..Default::default()
            };
            let (pg2, store2, storage2) = (pg.clone(), store.clone(), storage.clone());
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = HttpServer::new(move || {
                App::new()
                    .app_data(web::Data::new(pg2.clone()))
                    .app_data(store2.clone())
                    .app_data(web::Data::new(storage2.clone()))
                    .route("/collab/{id}", web::get().to(ws_handler))
            })
            .workers(1)
            .listen(listener)
            .unwrap()
            .run();
            let handle = server.handle();
            actix_web::rt::spawn(server);
            Self {
                port,
                id,
                owner,
                store,
                pg,
                storage,
                handle,
            }
        }
        async fn connect(&self, readonly: bool) -> Client {
            let token =
                auth_core::ws_capability::create_ws_capability(self.owner, self.id, readonly)
                    .unwrap();
            let (mut client, status) = Client::request(
                self.port,
                &format!("/collab/{}?ticket={token}", self.id),
                true,
            )
            .await;
            assert!(status.contains("101"), "{status}");
            client.frame().await.unwrap();
            client
        }
        fn content(&self) -> String {
            let room = self.store.get(&self.id).unwrap();
            let doc = room.doc.read().unwrap();
            doc.get_or_insert_text("content")
                .get_string(&doc.transact())
        }
        async fn stop(self) {
            self.handle.stop(false).await;
        }
    }
    struct Client(TcpStream);
    impl Client {
        async fn request(port: u16, path: &str, upgrade: bool) -> (Self, String) {
            let mut tcp = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            let headers = if upgrade {
                "Upgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n"
            } else {
                "Connection: close\r\n"
            };
            tcp.write_all(
                format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n{headers}\r\n").as_bytes(),
            )
            .await
            .unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                header.push(tcp.read_u8().await.unwrap());
            }
            (Self(tcp), String::from_utf8(header).unwrap())
        }
        async fn send(&mut self, opcode: u8, final_frame: bool, bytes: &[u8]) {
            let mut frame = vec![(if final_frame { 128 } else { 0 }) | opcode];
            if bytes.len() < 126 {
                frame.push(128 | bytes.len() as u8);
            } else if bytes.len() <= 65535 {
                frame.push(128 | 126);
                frame.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
            } else {
                frame.push(128 | 127);
                frame.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
            }
            frame.extend_from_slice(&[0, 0, 0, 0]);
            frame.extend_from_slice(bytes);
            self.0.write_all(&frame).await.unwrap();
        }
        async fn frame(&mut self) -> Option<(u8, Vec<u8>)> {
            tokio::time::timeout(Duration::from_millis(500), async {
                let opcode = self.0.read_u8().await.ok()? & 15;
                let len = self.0.read_u8().await.ok()? & 127;
                let len = match len {
                    126 => self.0.read_u16().await.ok()? as usize,
                    127 => self.0.read_u64().await.ok()? as usize,
                    n => n as usize,
                };
                let mut bytes = vec![0; len];
                self.0.read_exact(&mut bytes).await.ok()?;
                Some((opcode, bytes))
            })
            .await
            .ok()
            .flatten()
        }
    }
    fn update(text: &str) -> Vec<u8> {
        let doc = Doc::new();
        doc.get_or_insert_text("content")
            .insert(&mut doc.transact_mut(), 0, text);
        encode_update(
            &doc.transact()
                .encode_state_as_update_v1(&StateVector::default()),
        )
    }

    /// REG-02 (#2): a room recreated after eviction restores persisted state.
    #[actix_web::test]
    async fn regression_02_rejoin_after_eviction_restores_content() {
        let env = Env::new(false).await;
        let mut client = env.connect(false).await;
        client.send(2, true, &update("persist me")).await;
        client.frame().await.unwrap();
        client.send(8, true, &[]).await;
        client.frame().await;
        let room = env.store.get(&env.id).unwrap().clone();
        assert_eq!(room.connection_count(), 0);
        *room.last_empty_at.lock().unwrap() =
            Some(std::time::Instant::now() - Duration::from_secs(301));
        collab_core::snapshot::eviction_sweep(&env.storage, &env.store).await;
        assert!(!env.storage.snapshots.lock().unwrap().is_empty());
        let _reader = env.connect(true).await;
        assert_eq!(env.content(), "persist me");
        env.stop().await;
    }

    /// REG-03 (#3): the server starts a two-way sync so an editor uploads
    /// changes made while it was disconnected.
    #[actix_web::test]
    async fn regression_03_reconnect_handshake_uploads_offline_edits() {
        let env = Env::new(false).await;
        let token =
            auth_core::ws_capability::create_ws_capability(env.owner, env.id, false).unwrap();
        let (mut client, status) = Client::request(
            env.port,
            &format!("/collab/{}?ticket={token}", env.id),
            true,
        )
        .await;
        assert!(status.contains("101"), "{status}");

        let (_, initial_sync) = client.frame().await.expect("server must initiate sync");
        let server_vector = match decode_message(&initial_sync) {
            CollabMessage::SyncStep1(vector) => vector,
            _ => panic!("editable connections must receive SyncStep1"),
        };

        let offline = Doc::new();
        offline.get_or_insert_text("content").insert(
            &mut offline.transact_mut(),
            0,
            "offline edit",
        );
        let upload = encode_sync_step2(&offline, &server_vector);
        client.send(2, true, &upload).await;
        client
            .frame()
            .await
            .expect("accepted update must be broadcast");

        assert_eq!(env.content(), "offline edit");
        assert_eq!(env.storage.ops.lock().unwrap().len(), 1);
        env.stop().await;
    }

    /// REG-05 (#5): existing plaintext initializes a new CRDT room once on
    /// the server, regardless of how many clients join.
    #[actix_web::test]
    async fn regression_05_server_initializes_content_once() {
        let env = Env::new(false).await;
        *env.pg.1.lock().unwrap() = Some("initial content".to_string());

        let _first = env.connect(false).await;
        let _second = env.connect(false).await;

        assert_eq!(env.content(), "initial content");
        env.stop().await;
    }

    /// REG-05 (#5): read-only clients cannot mutate; their updates are
    /// ignored without severing the connection.
    #[actix_web::test]
    async fn regression_05_readonly_update_is_ignored_without_closing() {
        let env = Env::new(false).await;
        let mut client = env.connect(true).await;
        client.send(2, true, &update("REST initial content")).await;
        assert!(client.frame().await.is_none(), "connection must stay open");
        assert_eq!(env.content(), "");
        env.stop().await;
    }

    /// REG-06 (#6): readers are disconnected when a document becomes private.
    #[actix_web::test]
    async fn regression_06_reader_disconnected_when_document_becomes_private() {
        let env = Env::new(false).await;
        let (mut reader, status) =
            Client::request(env.port, &format!("/collab/{}", env.id), true).await;
        assert!(status.contains("101"));
        reader.frame().await.unwrap();
        env.pg.0.lock().unwrap().as_mut().unwrap().is_public = false;
        let (opcode, _) = reader.frame().await.expect("reader must be closed");
        assert_eq!(opcode, 8);
        env.stop().await;
    }

    /// REG-06 (#6): editors are disconnected when the document is deleted.
    #[actix_web::test]
    async fn regression_06_editor_disconnected_after_document_deletion() {
        let env = Env::new(false).await;
        let mut client = env.connect(false).await;
        *env.pg.0.lock().unwrap() = None;
        let (opcode, _) = client.frame().await.expect("editor must be closed");
        assert_eq!(opcode, 8);
        env.stop().await;
    }

    /// REG-21 (#21): a client that fell behind the broadcast buffer is
    /// resynchronized automatically; it must converge without manual action.
    #[actix_web::test]
    async fn regression_21_lagged_client_is_resynchronized_automatically() {
        let env = Env::new(false).await;
        let mut client = env.connect(false).await;
        let release = Arc::new(tokio::sync::Notify::new());
        *env.storage.pause.lock().unwrap() = Some(release.clone());
        client.send(2, true, &update("")).await;
        tokio::time::timeout(Duration::from_secs(1), env.storage.entered.notified())
            .await
            .unwrap();
        let sender = Doc::new();
        let t = sender.get_or_insert_text("content");
        t.insert(&mut sender.transact_mut(), 0, "A");
        let first = sender
            .transact()
            .encode_state_as_update_v1(&StateVector::default());
        let vector = sender.transact().state_vector();
        t.insert(&mut sender.transact_mut(), 1, "B");
        let next = sender.transact().encode_state_as_update_v1(&vector);
        {
            let room = env.store.get(&env.id).unwrap();
            apply_update_safe(&room.doc.read().unwrap(), &first).unwrap();
            apply_update_safe(&room.doc.read().unwrap(), &next).unwrap();
            room.tx.send(encode_update(&first).into()).unwrap();
            for _ in 0..256 {
                room.tx.send(encode_update(&next).into()).unwrap();
            }
        }
        release.notify_one();
        client.send(9, true, b"wake").await;
        assert_eq!(client.frame().await.unwrap().0, 10);
        let replica = Doc::new();
        while let Some((_, bytes)) = client.frame().await {
            match decode_message(&bytes) {
                CollabMessage::Update(data) | CollabMessage::SyncStep2(data) => {
                    apply_update_safe(&replica, &data).unwrap();
                }
                _ => {}
            }
        }
        let text = replica.get_or_insert_text("content");
        assert_eq!(text.get_string(&replica.transact()), "AB");
        env.stop().await;
    }

    /// REG-22 (#22): updates within the advertised 100 KiB message limit are
    /// accepted, broadcast, and do not silently break the connection.
    #[actix_web::test]
    async fn regression_22_update_under_message_limit_is_applied() {
        let env = Env::new(false).await;
        let mut client = env.connect(false).await;
        let payload = "x".repeat(70_000);
        let large = update(&payload);
        assert!(large.len() < MAX_MSG_BYTES);
        client.send(2, true, &large).await;
        let (_, bytes) = client.frame().await.expect("must be broadcast");
        assert!(matches!(decode_message(&bytes), CollabMessage::Update(_)));
        assert_eq!(env.content(), payload);
        env.stop().await;
    }

    /// REG-22 (#22): fragmented binary updates are aggregated and applied.
    #[actix_web::test]
    async fn regression_22_fragmented_update_is_aggregated() {
        let env = Env::new(false).await;
        let mut client = env.connect(false).await;
        let bytes = update("fragmented");
        let mid = bytes.len() / 2;
        client.send(2, false, &bytes[..mid]).await;
        client.send(0, true, &bytes[mid..]).await;
        let (_, bytes) = client.frame().await.expect("must be broadcast");
        assert!(matches!(decode_message(&bytes), CollabMessage::Update(_)));
        assert_eq!(env.content(), "fragmented");
        env.stop().await;
    }

    /// REG-23 (#23): failed upgrades must not consume editor slots.
    #[actix_web::test]
    async fn regression_23_failed_upgrades_do_not_consume_slots() {
        let env = Env::new(false).await;
        for _ in 0..100 {
            let (_, status) =
                Client::request(env.port, &format!("/collab/{}", env.id), false).await;
            assert!(status.contains("400"), "{status}");
        }
        assert_eq!(env.store.get(&env.id).unwrap().connection_count(), 0);
        let (_, status) = Client::request(env.port, &format!("/collab/{}", env.id), true).await;
        assert!(status.contains("101"), "{status}");
        env.stop().await;
    }

    /// REG-24 (#24): a malformed frame ends the session cleanly and releases
    /// its connection slot.
    #[actix_web::test]
    async fn regression_24_malformed_frame_releases_connection_slot() {
        let env = Env::new(false).await;
        let mut client = env.connect(false).await;
        let mut bytes = vec![0, 2];
        bytes.extend_from_slice(&[255, 255, 255, 255, 255, 255, 255, 255, 255, 1]);
        client.send(2, true, &bytes).await;
        assert!(client.frame().await.is_none());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(env.store.get(&env.id).unwrap().connection_count(), 0);
        env.stop().await;
    }

    /// REG-25 (#25): the document size limit is enforced at the update path.
    #[actix_web::test]
    async fn regression_25_document_size_limit_is_enforced() {
        let env = Env::new(false).await;
        let mut client = env.connect(false).await;
        for _ in 0..22 {
            client.send(2, true, &update(&"x".repeat(50_000))).await;
            let _ = client.frame().await;
        }
        assert!(env.content().len() <= MAX_DOC_BYTES);
        env.stop().await;
    }

    /// REG-29 (#29): an update that fails WAL persistence must not be
    /// broadcast as durable collaboration.
    #[actix_web::test]
    async fn regression_29_failed_wal_write_is_not_broadcast() {
        let env = Env::new(true).await;
        let mut client = env.connect(false).await;
        client.send(2, true, &update("not durable")).await;
        assert!(
            client.frame().await.is_none(),
            "non-durable update must not be broadcast"
        );
        assert_eq!(env.content(), "", "non-durable update must not be applied");
        assert!(env.storage.ops.lock().unwrap().is_empty());
        assert_eq!(env.pg.1.lock().unwrap().as_deref(), Some(""));
        env.stop().await;
    }

    /// Guard: PostgreSQL projection failure does not reject an update already
    /// durable in the WAL; recovery can rebuild the projection later.
    #[actix_web::test]
    async fn guard_projection_failure_keeps_durable_update_accepted() {
        let env = Env::new_with_failures(false, true).await;
        let mut client = env.connect(false).await;
        client.send(2, true, &update("durable")).await;
        client
            .frame()
            .await
            .expect("durable update must still be broadcast");

        assert_eq!(env.content(), "durable");
        assert_eq!(env.storage.ops.lock().unwrap().len(), 1);
        assert_eq!(env.pg.1.lock().unwrap().as_deref(), Some(""));
        env.stop().await;
    }

    /// Guard: the 100-editor cap rejects the 101st concurrent upgrade with
    /// HTTP 429 (end-to-end through the real handler, not just the counter).
    #[actix_web::test]
    async fn guard_editor_cap_rejects_101st_connection() {
        let env = Env::new(false).await;
        let mut clients = Vec::new();
        for _ in 0..collab_core::room::MAX_EDITORS {
            clients.push(env.connect(false).await);
        }
        let token =
            auth_core::ws_capability::create_ws_capability(env.owner, env.id, false).unwrap();
        let (_, status) = Client::request(
            env.port,
            &format!("/collab/{}?ticket={token}", env.id),
            true,
        )
        .await;
        assert!(
            status.contains("429"),
            "101st editor must be rejected: {status}"
        );
        drop(clients);
        env.stop().await;
    }
}
