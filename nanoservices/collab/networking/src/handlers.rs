use actix_web::{HttpRequest, HttpResponse, web};
use bytes::Bytes;
use collab_core::room::{AwarenessPeer, DocRoom, MAX_MSG_BYTES, get_or_create_room};
use collab_core::snapshot::{persist_snapshot, restore_room};
use collab_core::{
    CollabMessage, DocStore, accept_update, decode_message, encode_full_sync_step2,
    encode_sync_step1, encode_sync_step2, encode_update,
};
use dal::{
    DeleteSnapshot, GetDocumentById, GetDocumentContent, GetDocumentMember, ProjectDocumentContent,
    ReadLatestSnapshot, ScyllaDescriptor, WriteOp, WriteSnapshot,
    postgres_txs::SqlxPostGresDescriptor,
};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::Value;
use std::sync::PoisonError;
use tracing::{info, warn};
use uuid::Uuid;
use yrs::sync::AwarenessUpdate;
use yrs::updates::decoder::Decode;
use yrs::{Text, Transact};

#[derive(Clone, Copy)]
struct ConnectionMeta {
    doc_id: Uuid,
    client_id: Uuid,
    user_id: Option<Uuid>,
    connection_id: u64,
    is_readonly: bool,
}

pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<Uuid>,
    query: web::Query<WsQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let doc_id = *path;

    let pg_dal = req
        .app_data::<web::Data<SqlxPostGresDescriptor>>()
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("DAL not configured"))?;

    // Authenticated connections use signed short-lived capability tokens.
    // Public no-ticket viewers still hit Postgres once to confirm the doc is public.
    let (user_id, is_readonly) = if let Some(raw_token) = &query.ticket {
        let claims = auth_core::ws_capability::verify_ws_capability(raw_token)
            .map_err(|_| actix_web::error::ErrorUnauthorized("Invalid or expired ticket"))?;

        if claims.doc_id != doc_id {
            return Ok(HttpResponse::Unauthorized().body("Ticket doc mismatch"));
        }

        let doc = pg_dal
            .get_document_by_id(doc_id)
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        let Some(doc) = doc else {
            return Ok(HttpResponse::NotFound().body("Document not found"));
        };

        let currently_readonly = if doc.owner_id == claims.sub {
            false
        } else if let Some(member) = pg_dal
            .get_document_member(doc_id, claims.sub)
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?
        {
            matches!(member.role, kernel::MemberRole::Viewer)
        } else if doc.is_public {
            true
        } else {
            return Ok(HttpResponse::Unauthorized().body("Document access revoked"));
        };

        // A stale capability may retain fewer privileges, but never more, than
        // the current document policy grants.
        (Some(claims.sub), claims.readonly || currently_readonly)
    } else {
        let doc = pg_dal
            .get_document_by_id(doc_id)
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

        match doc {
            Some(doc) if doc.is_public => (None, true),
            Some(_) => return Ok(HttpResponse::Unauthorized().body("Document is not public")),
            None => return Ok(HttpResponse::NotFound().body("Document not found")),
        }
    };

    let scylla_dal = req
        .app_data::<web::Data<ScyllaDescriptor>>()
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("Scylla DAL not configured"))?
        .get_ref()
        .clone();

    let doc_store = req
        .app_data::<web::Data<DocStore>>()
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("DocStore not configured"))?
        .clone();

    let room = get_or_create_room(&doc_store, doc_id);
    room.ensure_initialized(|| async {
        if !restore_room(&scylla_dal, doc_id, &room).await? {
            if let Some(content) = pg_dal.get_document_content(doc_id).await? {
                if !content.is_empty() {
                    let doc = room.doc.write().unwrap();
                    doc.get_or_insert_text("content")
                        .insert(&mut doc.transact_mut(), 0, &content);
                }
            }
        }
        Ok::<(), utils::errors::NanoServiceError>(())
    })
    .await
    .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    // Validate the WebSocket upgrade before consuming a room connection slot.
    let (response, mut session, msg_stream) = actix_ws::handle(&req, stream)?;

    if !room.add_connection() {
        return Ok(HttpResponse::TooManyRequests().body("Editor cap reached (max 100)"));
    }

    let mut msg_stream = msg_stream
        .max_frame_size(MAX_MSG_BYTES)
        .aggregate_continuations()
        .max_continuation_size(MAX_MSG_BYTES);

    let room_clone = room.clone();
    let pg_projection = pg_dal.get_ref().clone();
    let mut broadcast_rx = room.tx.subscribe();
    let mut access_rx = room.subscribe_to_access_changes();
    let connection_id = Uuid::new_v4().as_u128() as u64;

    actix_web::rt::spawn(async move {
        let client_id = user_id.unwrap_or_else(Uuid::new_v4);
        let meta = ConnectionMeta {
            doc_id,
            client_id,
            user_id,
            connection_id,
            is_readonly,
        };

        // Editors exchange state vectors in both directions so reconnecting
        // clients upload offline edits. Read-only clients only receive state.
        {
            let initial_sync = {
                let doc = room_clone.doc.read().unwrap();
                if is_readonly {
                    encode_full_sync_step2(&doc)
                } else {
                    encode_sync_step1(&doc)
                }
            };
            let _ = session.binary(Bytes::from(initial_sync)).await;
        }

        info!(doc_id = %doc_id, client_id = %client_id, "WS connected");

        loop {
            tokio::select! {
                // Incoming message from this client
                msg = msg_stream.next() => {
                    match msg {
                        Some(Ok(actix_ws::AggregatedMessage::Binary(data))) => {
                            if data.len() > MAX_MSG_BYTES {
                                warn!(doc_id = %doc_id, "message too large ({} bytes), dropping", data.len());
                                break;
                            }
                            handle_binary(
                                &data,
                                meta,
                                &room_clone,
                                &mut session,
                                &scylla_dal,
                                &pg_projection,
                            )
                            .await;
                        }
                        Some(Ok(actix_ws::AggregatedMessage::Ping(payload))) => {
                            let _ = session.pong(&payload).await;
                        }
                        Some(Ok(actix_ws::AggregatedMessage::Close(_))) | None => break,
                        Some(Err(error)) => {
                            warn!(doc_id = %doc_id, %error, "WebSocket protocol error; closing client");
                            break;
                        }
                        Some(Ok(_)) => {}
                    }
                }
                // Broadcast from other clients
                broadcast = broadcast_rx.recv() => {
                    match broadcast {
                        Ok(bytes) => {
                            let _ = session.binary(bytes).await;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            warn!(doc_id = %doc_id, skipped, "client lagged; sending full resync");
                            // Start a fresh subscription before snapshotting. Updates concurrent
                            // with the snapshot are then either included in it, queued afterward,
                            // or both (CRDT updates are idempotent).
                            broadcast_rx = room_clone.tx.subscribe();
                            let full_sync = {
                                let doc = room_clone
                                    .doc
                                    .read()
                                    .unwrap_or_else(PoisonError::into_inner);
                                encode_full_sync_step2(&doc)
                            };
                            if session.binary(Bytes::from(full_sync)).await.is_err() {
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                access_change = access_rx.recv() => {
                    match access_change {
                        Ok(change) if change.disconnects(meta.user_id) => break,
                        Ok(_) => {}
                        // Missing an authorization event is unsafe, so force a
                        // reconnect that revalidates access against Postgres.
                        Err(_) => break,
                    }
                }
            }
        }

        room_clone.remove_connection_awareness(connection_id);
        room_clone.remove_connection();
        let _ = session.close(None).await;
        info!(doc_id = %doc_id, client_id = %client_id, "WS disconnected");
    });

    Ok(response)
}

async fn handle_binary<D, P>(
    data: &[u8],
    meta: ConnectionMeta,
    room: &DocRoom,
    session: &mut actix_ws::Session,
    dal: &D,
    projection: &P,
) where
    D: WriteOp + WriteSnapshot + ReadLatestSnapshot + DeleteSnapshot,
    P: ProjectDocumentContent,
{
    match decode_message(data) {
        CollabMessage::SyncStep1(sv_bytes) => {
            let step2 = {
                let doc = room.doc.read().unwrap();
                encode_sync_step2(&doc, &sv_bytes)
            };
            let _ = session.binary(Bytes::from(step2)).await;
        }
        CollabMessage::Update(update_bytes) | CollabMessage::SyncStep2(update_bytes) => {
            if meta.is_readonly {
                return;
            }
            match accept_update(
                dal,
                projection,
                meta.doc_id,
                meta.client_id,
                room,
                &update_bytes,
            )
            .await
            {
                Ok(Some(accepted)) => {
                    let broadcast_msg = Bytes::from(encode_update(&accepted.data));
                    let _ = room.tx.send(broadcast_msg);

                    if room.should_snapshot() {
                        persist_snapshot(dal, meta.doc_id, room).await;
                    }
                }
                Ok(None) => {
                    warn!(doc_id = %meta.doc_id, "malformed update bytes, dropping client");
                }
                Err(error) => {
                    warn!(doc_id = %meta.doc_id, %error, "WAL write failed; update rejected");
                }
            }
        }
        CollabMessage::Awareness(aw_bytes) => {
            let updates = awareness_updates_from_bytes(&aw_bytes, meta.user_id);
            if !updates.is_empty() {
                room.apply_awareness_update(meta.connection_id, updates);
            }
            // Forward awareness to all other clients
            let mut buf = vec![1u8];
            let len = aw_bytes.len();
            // write varint length
            let mut n = len;
            loop {
                let b = (n & 0x7F) as u8;
                n >>= 7;
                if n == 0 {
                    buf.push(b);
                    break;
                } else {
                    buf.push(b | 0x80);
                }
            }
            buf.extend_from_slice(&aw_bytes);
            let _ = room.tx.send(Bytes::from(buf));
        }
        CollabMessage::Unknown => {}
    }
}

fn awareness_updates_from_bytes(
    data: &[u8],
    user_id: Option<Uuid>,
) -> Vec<(u64, u32, Option<AwarenessPeer>)> {
    let update = match AwarenessUpdate::decode_v1(data) {
        Ok(update) => update,
        Err(_) => return Vec::new(),
    };

    update
        .clients
        .into_iter()
        .filter_map(|(client_id, entry)| {
            if entry.json.as_ref() == "null" {
                return Some((client_id, entry.clock, None));
            }

            let payload: AwarenessUserEnvelope = serde_json::from_str(&entry.json).ok()?;
            Some((
                client_id,
                entry.clock,
                Some(AwarenessPeer {
                    user_id,
                    name: payload.user.name,
                    color: payload.user.color,
                    last_active_ms: payload.user.last_active,
                }),
            ))
        })
        .collect()
}

#[derive(Deserialize)]
struct AwarenessUserEnvelope {
    user: AwarenessUser,
}

#[derive(Deserialize)]
struct AwarenessUser {
    name: String,
    color: String,
    #[serde(rename = "lastActive")]
    #[serde(deserialize_with = "deserialize_i64")]
    last_active: i64,
}

fn deserialize_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Number(n) => n
            .as_i64()
            .ok_or_else(|| serde::de::Error::custom("expected i64 number")),
        Value::String(s) => s
            .parse::<i64>()
            .map_err(|_| serde::de::Error::custom("expected i64 string")),
        _ => Err(serde::de::Error::custom("expected integer lastActive")),
    }
}

#[derive(serde::Deserialize)]
pub struct WsQuery {
    pub ticket: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use yrs::sync::AwarenessUpdate;
    use yrs::sync::awareness::AwarenessUpdateEntry;
    use yrs::updates::encoder::Encode;

    #[test]
    fn awareness_updates_from_bytes_reads_user_payload() {
        let mut clients = std::collections::HashMap::new();
        clients.insert(
            7,
            AwarenessUpdateEntry {
                clock: 1,
                json: r##"{"user":{"name":"alice","color":"#E53E3E","lastActive":1700000000000}}"##
                    .into(),
            },
        );
        let bytes = AwarenessUpdate { clients }.encode_v1();

        let authenticated_user_id = Uuid::new_v4();
        let updates = awareness_updates_from_bytes(&bytes, Some(authenticated_user_id));
        let (client_id, clock, peer) = &updates[0];
        let peer = peer.as_ref().unwrap();
        assert_eq!(*client_id, 7);
        assert_eq!(*clock, 1);
        assert_eq!(peer.user_id, Some(authenticated_user_id));
        assert_eq!(peer.name, "alice");
        assert_eq!(peer.color, "#E53E3E");
        assert_eq!(peer.last_active_ms, 1_700_000_000_000);
    }

    #[test]
    fn awareness_updates_from_bytes_preserves_null_state() {
        let mut clients = std::collections::HashMap::new();
        clients.insert(
            7,
            AwarenessUpdateEntry {
                clock: 2,
                json: "null".into(),
            },
        );
        let bytes = AwarenessUpdate { clients }.encode_v1();
        let updates = awareness_updates_from_bytes(&bytes, Some(Uuid::new_v4()));
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].0, 7);
        assert_eq!(updates[0].1, 2);
        assert!(updates[0].2.is_none());
    }

    #[test]
    fn awareness_updates_from_bytes_keeps_multiple_clients() {
        let mut clients = std::collections::HashMap::new();
        clients.insert(
            7,
            AwarenessUpdateEntry {
                clock: 1,
                json: r##"{"user":{"name":"alice","color":"#E53E3E","lastActive":1700000000000}}"##
                    .into(),
            },
        );
        clients.insert(
            9,
            AwarenessUpdateEntry {
                clock: 1,
                json: r##"{"user":{"name":"bob","color":"#3182CE","lastActive":1700000001000}}"##
                    .into(),
            },
        );
        let bytes = AwarenessUpdate { clients }.encode_v1();
        let updates = awareness_updates_from_bytes(&bytes, None);
        assert_eq!(updates.len(), 2);
        assert!(
            updates
                .iter()
                .all(|(_, _, peer)| peer.as_ref().unwrap().user_id.is_none())
        );
    }
}
