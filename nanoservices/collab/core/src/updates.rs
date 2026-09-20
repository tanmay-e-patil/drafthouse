use chrono::Utc;
use dal::{ProjectDocumentContent, WriteOp};
use kernel::NewCollabOp;
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};
use uuid::Uuid;
use yrs::{GetString, Transact, Update, updates::decoder::Decode};

use crate::{room::DocRoom, sync_protocol::apply_update_safe};

pub struct AcceptedUpdate {
    pub sequence: i64,
    pub data: Vec<u8>,
}

/// Durably accept one collaboration update under the room's ordering gate.
///
/// WAL persistence is authoritative and must succeed before the in-memory
/// document changes. PostgreSQL plaintext is a revision-guarded projection;
/// projection failure does not invalidate an update already durable in WAL.
pub async fn accept_update<W, P>(
    wal: &W,
    projection: &P,
    doc_id: Uuid,
    client_id: Uuid,
    room: &DocRoom,
    update_bytes: &[u8],
) -> Result<Option<AcceptedUpdate>, NanoServiceError>
where
    W: WriteOp,
    P: ProjectDocumentContent,
{
    if Update::decode_v1(update_bytes).is_err() {
        return Ok(None);
    }

    room.serialize_update(|| async {
        if room.is_closed() {
            return Err(NanoServiceError::new(
                "Document deleted; update rejected",
                NanoServiceErrorStatus::NotFound,
            ));
        }

        let sequence = room.next_operation_sequence();
        wal.write_op(NewCollabOp {
            doc_id,
            sequence,
            op_id: Uuid::new_v4(),
            client_id,
            data: update_bytes.to_vec(),
            created_at: Utc::now(),
        })
        .await?;

        let data = apply_update_safe(&room.doc.read().unwrap(), update_bytes).ok_or_else(|| {
            NanoServiceError::new(
                "Failed to apply persisted collaboration update",
                NanoServiceErrorStatus::InternalServerError,
            )
        })?;

        let content = {
            let doc = room.doc.read().unwrap();
            doc.get_or_insert_text("content")
                .get_string(&doc.transact())
        };
        if let Err(error) = projection
            .project_document_content(doc_id, content, sequence)
            .await
        {
            tracing::warn!(doc_id = %doc_id, sequence, %error, "content projection failed");
        }

        room.increment_ops();
        Ok(Some(AcceptedUpdate { sequence, data }))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use yrs::{Doc, ReadTxn, StateVector, Text};

    #[derive(Clone, Default)]
    struct Wal {
        writes: Arc<Mutex<Vec<(i64, Vec<u8>)>>>,
        fail: bool,
    }

    impl WriteOp for Wal {
        async fn write_op(&self, op: NewCollabOp) -> Result<(), NanoServiceError> {
            if self.fail {
                return Err(NanoServiceError::new(
                    "injected WAL failure",
                    NanoServiceErrorStatus::InternalServerError,
                ));
            }
            self.writes.lock().unwrap().push((op.sequence, op.data));
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct Projection {
        value: Arc<Mutex<Option<(i64, String)>>>,
        fail: bool,
    }

    impl ProjectDocumentContent for Projection {
        async fn project_document_content(
            &self,
            _: Uuid,
            content: String,
            revision: i64,
        ) -> Result<bool, NanoServiceError> {
            if self.fail {
                return Err(NanoServiceError::new(
                    "injected projection failure",
                    NanoServiceErrorStatus::InternalServerError,
                ));
            }
            let mut value = self.value.lock().unwrap();
            if value
                .as_ref()
                .is_some_and(|(current_revision, _)| *current_revision >= revision)
            {
                return Ok(false);
            }
            *value = Some((revision, content));
            Ok(true)
        }
    }

    fn incremental_updates() -> (Vec<u8>, Vec<u8>) {
        let doc = Doc::new();
        let text = doc.get_or_insert_text("content");
        text.insert(&mut doc.transact_mut(), 0, "A");
        let after_a = doc.transact().state_vector();
        let first = doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default());
        text.insert(&mut doc.transact_mut(), 1, "B");
        let second = doc.transact().encode_state_as_update_v1(&after_a);
        (first, second)
    }

    #[tokio::test]
    async fn wal_failure_does_not_mutate_room_or_projection() {
        let wal = Wal {
            fail: true,
            ..Default::default()
        };
        let projection = Projection::default();
        let room = DocRoom::new();
        let (update, _) = incremental_updates();

        assert!(
            accept_update(
                &wal,
                &projection,
                Uuid::new_v4(),
                Uuid::new_v4(),
                &room,
                &update,
            )
            .await
            .is_err()
        );
        assert_eq!(room.current_sequence(), 1, "failed sequence remains a gap");
        let content = {
            let doc = room.doc.read().unwrap();
            doc.get_or_insert_text("content")
                .get_string(&doc.transact())
        };
        assert_eq!(content, "");
        assert!(projection.value.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn concurrent_updates_receive_unique_sequences_and_project_merged_state() {
        let wal = Wal::default();
        let projection = Projection::default();
        let room = DocRoom::new();
        let doc_id = Uuid::new_v4();
        let (first, second) = incremental_updates();

        let (a, b) = tokio::join!(
            accept_update(&wal, &projection, doc_id, Uuid::new_v4(), &room, &first),
            accept_update(&wal, &projection, doc_id, Uuid::new_v4(), &room, &second),
        );
        assert!(a.unwrap().is_some());
        assert!(b.unwrap().is_some());

        let writes = wal.writes.lock().unwrap();
        assert_eq!(
            writes
                .iter()
                .map(|(sequence, _)| *sequence)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            projection.value.lock().unwrap().as_ref().unwrap(),
            &(2, "AB".to_string())
        );
    }

    #[tokio::test]
    async fn projection_failure_keeps_wal_update_accepted_and_recoverable() {
        let wal = Wal::default();
        let projection = Projection {
            fail: true,
            ..Default::default()
        };
        let room = DocRoom::new();
        let (update, _) = incremental_updates();

        let accepted = accept_update(
            &wal,
            &projection,
            Uuid::new_v4(),
            Uuid::new_v4(),
            &room,
            &update,
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(accepted.sequence, 1);
        assert_eq!(wal.writes.lock().unwrap().len(), 1);
    }
}
