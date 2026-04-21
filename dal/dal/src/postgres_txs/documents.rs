use super::auth::SqlxPostGresDescriptor;
use crate::documents_txs::{
    CountDocumentsByOwner, CreateDocument, CreateWsTicket, DeleteDocument, DeleteWsTicket,
    GetDocumentById, GetDocumentContent, GetWsTicketByHash, ListDocumentsByOwner, UpdateDocument,
    UpdateDocumentContent,
};
use dal_tx_impl::impl_transaction;
use kernel::{Document, NewDocument, NewWsTicket, WsTicket};
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

#[impl_transaction(SqlxPostGresDescriptor, CreateDocument, create_document)]
async fn create_document(&self, new_doc: NewDocument) -> Result<Document, NanoServiceError> {
    let row = sqlx::query_as::<_, Document>(
        "INSERT INTO documents (owner_id, title) VALUES ($1, $2) RETURNING id, owner_id, title, is_public, created_at, updated_at",
    )
    .bind(new_doc.owner_id)
    .bind(&new_doc.title)
    .fetch_one(&self.pool)
    .await
    .map_err(|e| NanoServiceError::new(format!("Failed to create document: {}", e), NanoServiceErrorStatus::InternalServerError))?;

    Ok(row)
}

#[impl_transaction(SqlxPostGresDescriptor, GetDocumentById, get_document_by_id)]
async fn get_document_by_id(&self, id: uuid::Uuid) -> Result<Option<Document>, NanoServiceError> {
    let row = sqlx::query_as::<_, Document>(
        "SELECT id, owner_id, title, is_public, created_at, updated_at FROM documents WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&self.pool)
    .await
    .map_err(|e| NanoServiceError::new(format!("Failed to get document: {}", e), NanoServiceErrorStatus::InternalServerError))?;

    Ok(row)
}

#[impl_transaction(SqlxPostGresDescriptor, UpdateDocument, update_document)]
async fn update_document(
    &self,
    id: uuid::Uuid,
    title: Option<String>,
    is_public: Option<bool>,
) -> Result<Document, NanoServiceError> {
    let row = sqlx::query_as::<_, Document>(
        "UPDATE documents SET title = COALESCE($2, title), is_public = COALESCE($3, is_public), updated_at = now() WHERE id = $1 RETURNING id, owner_id, title, is_public, created_at, updated_at",
    )
    .bind(id)
    .bind(&title)
    .bind(is_public)
    .fetch_one(&self.pool)
    .await
    .map_err(|e| NanoServiceError::new(format!("Failed to update document: {}", e), NanoServiceErrorStatus::InternalServerError))?;

    Ok(row)
}

#[impl_transaction(SqlxPostGresDescriptor, DeleteDocument, delete_document)]
async fn delete_document(&self, id: uuid::Uuid) -> Result<(), NanoServiceError> {
    utils::safe_eject!(
        sqlx::query("DELETE FROM documents WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await,
        NanoServiceErrorStatus::InternalServerError,
        "Failed to delete document"
    )?;
    Ok(())
}

#[impl_transaction(SqlxPostGresDescriptor, ListDocumentsByOwner, list_documents_by_owner)]
async fn list_documents_by_owner(
    &self,
    owner_id: uuid::Uuid,
    cursor: Option<uuid::Uuid>,
    limit: i64,
) -> Result<Vec<Document>, NanoServiceError> {
    let rows = if let Some(cursor_id) = cursor {
        sqlx::query_as::<_, Document>(
            "SELECT id, owner_id, title, is_public, created_at, updated_at FROM documents WHERE owner_id = $1 AND updated_at < (SELECT updated_at FROM documents WHERE id = $2) ORDER BY updated_at DESC LIMIT $3",
        )
        .bind(owner_id)
        .bind(cursor_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    } else {
        sqlx::query_as::<_, Document>(
            "SELECT id, owner_id, title, is_public, created_at, updated_at FROM documents WHERE owner_id = $1 ORDER BY updated_at DESC LIMIT $2",
        )
        .bind(owner_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }
    .map_err(|e| NanoServiceError::new(format!("Failed to list documents: {}", e), NanoServiceErrorStatus::InternalServerError))?;

    Ok(rows)
}

#[impl_transaction(
    SqlxPostGresDescriptor,
    CountDocumentsByOwner,
    count_documents_by_owner
)]
async fn count_documents_by_owner(&self, owner_id: uuid::Uuid) -> Result<i64, NanoServiceError> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM documents WHERE owner_id = $1")
        .bind(owner_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            NanoServiceError::new(
                format!("Failed to count documents: {}", e),
                NanoServiceErrorStatus::InternalServerError,
            )
        })?;

    Ok(row.0)
}

#[impl_transaction(SqlxPostGresDescriptor, GetDocumentContent, get_document_content)]
async fn get_document_content(&self, id: uuid::Uuid) -> Result<Option<String>, NanoServiceError> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT content FROM documents WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                NanoServiceError::new(
                    format!("Failed to get document content: {}", e),
                    NanoServiceErrorStatus::InternalServerError,
                )
            })?;

    Ok(row.map(|(c,)| c.unwrap_or_default()))
}

#[impl_transaction(SqlxPostGresDescriptor, UpdateDocumentContent, update_document_content)]
async fn update_document_content(
    &self,
    id: uuid::Uuid,
    content: String,
) -> Result<(), NanoServiceError> {
    utils::safe_eject!(
        sqlx::query("UPDATE documents SET content = $1, updated_at = now() WHERE id = $2")
            .bind(&content)
            .bind(id)
            .execute(&self.pool)
            .await,
        NanoServiceErrorStatus::InternalServerError,
        "Failed to update document content"
    )?;
    Ok(())
}

#[impl_transaction(SqlxPostGresDescriptor, CreateWsTicket, create_ws_ticket)]
async fn create_ws_ticket(&self, new_ticket: NewWsTicket) -> Result<WsTicket, NanoServiceError> {
    let row = sqlx::query_as::<_, WsTicket>(
        "INSERT INTO ws_tickets (token_hash, doc_id, user_id, expires_at) VALUES ($1, $2, $3, $4) RETURNING token_hash, doc_id, user_id, expires_at",
    )
    .bind(&new_ticket.token_hash)
    .bind(new_ticket.doc_id)
    .bind(new_ticket.user_id)
    .bind(new_ticket.expires_at)
    .fetch_one(&self.pool)
    .await
    .map_err(|e| NanoServiceError::new(format!("Failed to create ws ticket: {}", e), NanoServiceErrorStatus::InternalServerError))?;
    Ok(row)
}

#[impl_transaction(SqlxPostGresDescriptor, GetWsTicketByHash, get_ws_ticket_by_hash)]
async fn get_ws_ticket_by_hash(
    &self,
    token_hash: String,
) -> Result<Option<WsTicket>, NanoServiceError> {
    let row = sqlx::query_as::<_, WsTicket>(
        "SELECT token_hash, doc_id, user_id, expires_at FROM ws_tickets WHERE token_hash = $1",
    )
    .bind(&token_hash)
    .fetch_optional(&self.pool)
    .await
    .map_err(|e| {
        NanoServiceError::new(
            format!("Failed to get ws ticket: {}", e),
            NanoServiceErrorStatus::InternalServerError,
        )
    })?;
    Ok(row)
}

#[impl_transaction(SqlxPostGresDescriptor, DeleteWsTicket, delete_ws_ticket)]
async fn delete_ws_ticket(&self, token_hash: String) -> Result<(), NanoServiceError> {
    utils::safe_eject!(
        sqlx::query("DELETE FROM ws_tickets WHERE token_hash = $1")
            .bind(&token_hash)
            .execute(&self.pool)
            .await,
        NanoServiceErrorStatus::InternalServerError,
        "Failed to delete ws ticket"
    )?;
    Ok(())
}
