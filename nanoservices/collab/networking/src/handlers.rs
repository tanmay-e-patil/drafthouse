use actix_web::{HttpRequest, HttpResponse, web};
use bytes::Bytes;
use chrono::Utc;
use collab_core::room::{AwarenessPeer, DocRoom, MAX_MSG_BYTES, get_or_create_room};
use collab_core::snapshot::persist_snapshot;
use collab_core::sync_protocol::apply_update_safe;
use collab_core::{
    CollabMessage, DocStore, decode_message, encode_full_sync_step2, encode_sync_step2,
    encode_update,
};
use dal::{
    DeleteSnapshot, GetDocumentById, ReadLatestSnapshot, ScyllaDescriptor, WriteOp, WriteSnapshot,
    postgres_txs::SqlxPostGresDescriptor,
};
use futures_util::StreamExt;
use kernel::NewCollabOp;
use serde::Deserialize;
use serde_json::Value;
use tracing::{info, warn};
use uuid::Uuid;
use yrs::sync::AwarenessUpdate;
use yrs::updates::decoder::Decode;

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

        (Some(claims.sub), claims.readonly)
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

    if !room.add_connection() {
        return Ok(HttpResponse::TooManyRequests().body("Editor cap reached (max 100)"));
    }

    // Perform WebSocket upgrade
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    let room_clone = room.clone();
    let mut broadcast_rx = room.tx.subscribe();
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

        // Send SyncStep1 to new client so it replies with its state vector
        {
            let step1 = {
                let doc = room_clone.doc.read().unwrap();
                encode_full_sync_step2(&doc)
            };
            let _ = session.binary(Bytes::from(step1)).await;
        }

        info!(doc_id = %doc_id, client_id = %client_id, "WS connected");

        loop {
            tokio::select! {
                // Incoming message from this client
                msg = msg_stream.next() => {
                    match msg {
                        Some(Ok(actix_ws::Message::Binary(data))) => {
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
                            )
                            .await;
                        }
                        Some(Ok(actix_ws::Message::Ping(payload))) => {
                            let _ = session.pong(&payload).await;
                        }
                        Some(Ok(actix_ws::Message::Close(_))) | None => break,
                        _ => {}
                    }
                }
                // Broadcast from other clients
                Ok(bytes) = broadcast_rx.recv() => {
                    let _ = session.binary(bytes).await;
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

async fn handle_binary<D>(
    data: &[u8],
    meta: ConnectionMeta,
    room: &DocRoom,
    session: &mut actix_ws::Session,
    dal: &D,
) where
    D: WriteOp + WriteSnapshot + ReadLatestSnapshot + DeleteSnapshot,
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
                let _ = session
                    .clone()
                    .close(Some(actix_ws::CloseReason {
                        code: actix_ws::CloseCode::Policy,
                        description: Some("Read-only viewers cannot edit".to_string()),
                    }))
                    .await;
                return;
            }
            let applied = {
                let doc = room.doc.read().unwrap();
                apply_update_safe(&doc, &update_bytes)
            };
            if let Some(update_bytes) = applied {
                // Write to WAL async (fire and forget)
                let op_id = Uuid::new_v4();
                let new_op = NewCollabOp {
                    doc_id: meta.doc_id,
                    op_id,
                    client_id: meta.client_id,
                    data: update_bytes.clone(),
                    created_at: Utc::now(),
                };
                let _ = dal.write_op(new_op).await;

                // Broadcast to room
                let broadcast_msg = Bytes::from(encode_update(&update_bytes));
                let _ = room.tx.send(broadcast_msg);

                // Snapshot trigger
                room.increment_ops();
                if room.should_snapshot() {
                    persist_snapshot(dal, meta.doc_id, room).await;
                }
            } else {
                warn!(doc_id = %meta.doc_id, "malformed update bytes, dropping client");
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
) -> Vec<(u64, Option<AwarenessPeer>)> {
    let update = match AwarenessUpdate::decode_v1(data) {
        Ok(update) => update,
        Err(_) => return Vec::new(),
    };

    update
        .clients
        .into_iter()
        .filter_map(|(client_id, entry)| {
            if entry.json.as_ref() == "null" {
                return Some((client_id, None));
            }

            let payload: AwarenessUserEnvelope = serde_json::from_str(&entry.json).ok()?;
            Some((
                client_id,
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
        let (client_id, peer) = &updates[0];
        let peer = peer.as_ref().unwrap();
        assert_eq!(*client_id, 7);
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
        assert!(updates[0].1.is_none());
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
                .all(|(_, peer)| peer.as_ref().unwrap().user_id.is_none())
        );
    }
}
