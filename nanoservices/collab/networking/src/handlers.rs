use actix_web::{HttpRequest, HttpResponse, web};
use bytes::Bytes;
use chrono::Utc;
use collab_core::room::{MAX_MSG_BYTES, get_or_create_room};
use collab_core::snapshot::persist_snapshot;
use collab_core::sync_protocol::apply_update_safe;
use collab_core::{
    CollabMessage, DocStore, decode_message, encode_full_sync_step2, encode_sync_step2,
    encode_update,
};
use dal::{
    DeleteSnapshot, DeleteWsTicket, GetWsTicketByHash, ReadLatestSnapshot, ScyllaDescriptor,
    WriteOp, WriteSnapshot, postgres_txs::SqlxPostGresDescriptor,
};
use futures_util::StreamExt;
use kernel::NewCollabOp;
use tracing::{info, warn};
use uuid::Uuid;

pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<Uuid>,
    query: web::Query<WsQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let doc_id = *path;

    // Validate ticket against Postgres (optional — public docs have no ticket)
    let user_id = if let Some(raw_token) = &query.ticket {
        let pg_dal = req
            .app_data::<web::Data<SqlxPostGresDescriptor>>()
            .ok_or_else(|| actix_web::error::ErrorInternalServerError("DAL not configured"))?;

        let token_hash = hash_token(raw_token);

        let ticket = pg_dal
            .get_ws_ticket_by_hash(token_hash.clone())
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

        match ticket {
            None => {
                return Ok(HttpResponse::Unauthorized().body("Invalid or expired ticket"));
            }
            Some(t) if t.expires_at < Utc::now() => {
                // burn expired ticket
                let _ = pg_dal.delete_ws_ticket(token_hash).await;
                return Ok(HttpResponse::Unauthorized().body("Ticket expired"));
            }
            Some(t) if t.doc_id != doc_id => {
                return Ok(HttpResponse::Unauthorized().body("Ticket doc mismatch"));
            }
            Some(t) => {
                // burn single-use ticket
                let _ = pg_dal.delete_ws_ticket(token_hash).await;
                Some(t.user_id)
            }
        }
    } else {
        // Unauthenticated — viewer only; check later if doc is public
        None
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
    let is_readonly = user_id.is_none();

    actix_web::rt::spawn(async move {
        let client_id = user_id.unwrap_or_else(Uuid::new_v4);

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
                                doc_id,
                                client_id,
                                is_readonly,
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

        room_clone.remove_connection();
        let _ = session.close(None).await;
        info!(doc_id = %doc_id, client_id = %client_id, "WS disconnected");
    });

    Ok(response)
}

async fn handle_binary<D>(
    data: &[u8],
    doc_id: Uuid,
    client_id: Uuid,
    is_readonly: bool,
    room: &collab_core::room::DocRoom,
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
            if is_readonly {
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
                    doc_id,
                    op_id,
                    client_id,
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
                    persist_snapshot(dal, doc_id, room).await;
                }
            } else {
                warn!(doc_id = %doc_id, "malformed update bytes, dropping client");
            }
        }
        CollabMessage::Awareness(aw_bytes) => {
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

fn hash_token(raw: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(raw.as_bytes());
    hex::encode(h.finalize())
}

#[derive(serde::Deserialize)]
pub struct WsQuery {
    pub ticket: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_token_is_deterministic() {
        let h1 = hash_token("abc123");
        let h2 = hash_token("abc123");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_token_is_not_plaintext() {
        let raw = "mysecrettoken";
        let hashed = hash_token(raw);
        assert_ne!(hashed, raw);
        assert_eq!(hashed.len(), 64); // SHA256 hex = 64 chars
    }
}
