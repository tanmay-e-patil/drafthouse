use super::auth::SqlxPostGresDescriptor;
use crate::documents_txs::{
    AcceptInviteLink, CountDocumentsByOwner, CreateDocument, CreateInviteLink, CreateWsTicket,
    DeleteDocument, DeleteDocumentMember, DeleteWsTicket, GetDocumentById, GetDocumentContent,
    GetDocumentMember, GetInviteLinkByToken, GetWsTicketByHash, ListActiveInviteLinks,
    ListDocumentMembers, ListDocumentsByOwner, ListDocumentsByOwnerNoPagination, RevokeInviteLink,
    UpdateDocument, UpdateDocumentContent, UpdateDocumentMemberRole,
};
use chrono::Utc;
use dal_tx_impl::impl_transaction;
use kernel::{
    Document, DocumentMember, InviteLink, MemberRole, NewDocument, NewInviteLink, NewWsTicket,
    WsTicket,
};
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
            "SELECT d.id, d.owner_id, d.title, d.is_public, d.created_at, d.updated_at
             FROM documents d
             WHERE (
               d.owner_id = $1
               OR EXISTS (
                 SELECT 1 FROM document_members dm
                 WHERE dm.doc_id = d.id AND dm.user_id = $1
               )
             )
             AND d.updated_at < (SELECT updated_at FROM documents WHERE id = $2)
             ORDER BY d.updated_at DESC
             LIMIT $3",
        )
        .bind(owner_id)
        .bind(cursor_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    } else {
        sqlx::query_as::<_, Document>(
            "SELECT d.id, d.owner_id, d.title, d.is_public, d.created_at, d.updated_at
             FROM documents d
             WHERE d.owner_id = $1
             OR EXISTS (
               SELECT 1 FROM document_members dm
               WHERE dm.doc_id = d.id AND dm.user_id = $1
             )
             ORDER BY d.updated_at DESC
             LIMIT $2",
        )
        .bind(owner_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }
    .map_err(|e| {
        NanoServiceError::new(
            format!("Failed to list documents: {}", e),
            NanoServiceErrorStatus::InternalServerError,
        )
    })?;

    Ok(rows)
}

#[impl_transaction(
    SqlxPostGresDescriptor,
    ListDocumentsByOwnerNoPagination,
    list_documents_by_owner_no_pagination
)]
async fn list_documents_by_owner_no_pagination(
    &self,
    owner_id: uuid::Uuid,
) -> Result<Vec<Document>, NanoServiceError> {
    let rows = sqlx::query_as::<_, Document>(
        "SELECT id, owner_id, title, is_public, created_at, updated_at
         FROM documents
         WHERE owner_id = $1
         ORDER BY updated_at DESC",
    )
    .bind(owner_id)
    .fetch_all(&self.pool)
    .await
    .map_err(|e| {
        NanoServiceError::new(
            format!("Failed to list owned documents for export: {}", e),
            NanoServiceErrorStatus::InternalServerError,
        )
    })?;

    Ok(rows)
}

#[impl_transaction(
    SqlxPostGresDescriptor,
    CountDocumentsByOwner,
    count_documents_by_owner
)]
async fn count_documents_by_owner(&self, owner_id: uuid::Uuid) -> Result<i64, NanoServiceError> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)
         FROM documents d
         WHERE d.owner_id = $1
         OR EXISTS (
           SELECT 1 FROM document_members dm
           WHERE dm.doc_id = d.id AND dm.user_id = $1
         )",
    )
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

#[impl_transaction(SqlxPostGresDescriptor, CreateInviteLink, create_invite_link)]
async fn create_invite_link(
    &self,
    new_link: NewInviteLink,
) -> Result<InviteLink, NanoServiceError> {
    let row = sqlx::query_as::<_, InviteLink>(
        "INSERT INTO invite_links (token, doc_id, role, created_by, max_uses, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING token, doc_id, role, created_by, max_uses, use_count, expires_at, revoked_at",
    )
    .bind(&new_link.token)
    .bind(new_link.doc_id)
    .bind(new_link.role)
    .bind(new_link.created_by)
    .bind(new_link.max_uses)
    .bind(new_link.expires_at)
    .fetch_one(&self.pool)
    .await
    .map_err(|e| {
        NanoServiceError::new(
            format!("Failed to create invite link: {}", e),
            NanoServiceErrorStatus::InternalServerError,
        )
    })?;
    Ok(row)
}

#[impl_transaction(SqlxPostGresDescriptor, GetInviteLinkByToken, get_invite_link_by_token)]
async fn get_invite_link_by_token(
    &self,
    token: String,
) -> Result<Option<InviteLink>, NanoServiceError> {
    let row = sqlx::query_as::<_, InviteLink>(
        "SELECT token, doc_id, role, created_by, max_uses, use_count, expires_at, revoked_at
         FROM invite_links WHERE token = $1",
    )
    .bind(&token)
    .fetch_optional(&self.pool)
    .await
    .map_err(|e| {
        NanoServiceError::new(
            format!("Failed to get invite link: {}", e),
            NanoServiceErrorStatus::InternalServerError,
        )
    })?;
    Ok(row)
}

#[impl_transaction(
    SqlxPostGresDescriptor,
    ListActiveInviteLinks,
    list_active_invite_links
)]
async fn list_active_invite_links(
    &self,
    doc_id: uuid::Uuid,
) -> Result<Vec<InviteLink>, NanoServiceError> {
    let rows = sqlx::query_as::<_, InviteLink>(
        "SELECT token, doc_id, role, created_by, max_uses, use_count, expires_at, revoked_at
         FROM invite_links
         WHERE doc_id = $1
           AND revoked_at IS NULL
           AND (expires_at IS NULL OR expires_at > NOW())",
    )
    .bind(doc_id)
    .fetch_all(&self.pool)
    .await
    .map_err(|e| {
        NanoServiceError::new(
            format!("Failed to list invite links: {}", e),
            NanoServiceErrorStatus::InternalServerError,
        )
    })?;
    Ok(rows)
}

#[impl_transaction(SqlxPostGresDescriptor, RevokeInviteLink, revoke_invite_link)]
async fn revoke_invite_link(&self, token: String) -> Result<(), NanoServiceError> {
    utils::safe_eject!(
        sqlx::query("UPDATE invite_links SET revoked_at = NOW() WHERE token = $1")
            .bind(&token)
            .execute(&self.pool)
            .await,
        NanoServiceErrorStatus::InternalServerError,
        "Failed to revoke invite link"
    )?;
    Ok(())
}

#[impl_transaction(SqlxPostGresDescriptor, AcceptInviteLink, accept_invite_link)]
async fn accept_invite_link(
    &self,
    token: String,
    user_id: uuid::Uuid,
) -> Result<DocumentMember, NanoServiceError> {
    let mut tx = self.pool.begin().await.map_err(|e| {
        NanoServiceError::new(
            format!("Failed to begin transaction: {}", e),
            NanoServiceErrorStatus::InternalServerError,
        )
    })?;

    let link: Option<InviteLink> = sqlx::query_as::<_, InviteLink>(
        "SELECT token, doc_id, role, created_by, max_uses, use_count, expires_at, revoked_at
         FROM invite_links WHERE token = $1 FOR UPDATE",
    )
    .bind(&token)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        NanoServiceError::new(
            format!("Failed to fetch invite link: {}", e),
            NanoServiceErrorStatus::InternalServerError,
        )
    })?;

    let link = link.ok_or_else(|| {
        NanoServiceError::new("Invite link not found", NanoServiceErrorStatus::NotFound)
    })?;

    if link.revoked_at.is_some() {
        return Err(NanoServiceError::new(
            "Invite link has been revoked",
            NanoServiceErrorStatus::Gone,
        ));
    }
    if link.expires_at.is_some_and(|e| e < Utc::now()) {
        return Err(NanoServiceError::new(
            "Invite link has expired",
            NanoServiceErrorStatus::Gone,
        ));
    }
    if link.max_uses.is_some_and(|m| link.use_count >= m) {
        return Err(NanoServiceError::new(
            "Invite link has reached its maximum uses",
            NanoServiceErrorStatus::Gone,
        ));
    }

    sqlx::query("UPDATE invite_links SET use_count = use_count + 1 WHERE token = $1")
        .bind(&token)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            NanoServiceError::new(
                format!("Failed to increment use count: {}", e),
                NanoServiceErrorStatus::InternalServerError,
            )
        })?;

    let member: DocumentMember = sqlx::query_as::<_, DocumentMember>(
        "INSERT INTO document_members (doc_id, user_id, role)
         VALUES ($1, $2, $3)
         ON CONFLICT (doc_id, user_id) DO UPDATE SET role = EXCLUDED.role
         RETURNING doc_id, user_id, role",
    )
    .bind(link.doc_id)
    .bind(user_id)
    .bind(link.role)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        NanoServiceError::new(
            format!("Failed to insert member: {}", e),
            NanoServiceErrorStatus::InternalServerError,
        )
    })?;

    tx.commit().await.map_err(|e| {
        NanoServiceError::new(
            format!("Failed to commit transaction: {}", e),
            NanoServiceErrorStatus::InternalServerError,
        )
    })?;

    Ok(member)
}

#[impl_transaction(SqlxPostGresDescriptor, ListDocumentMembers, list_document_members)]
async fn list_document_members(
    &self,
    doc_id: uuid::Uuid,
) -> Result<Vec<DocumentMember>, NanoServiceError> {
    let rows = sqlx::query_as::<_, DocumentMember>(
        "SELECT doc_id, user_id, role FROM document_members WHERE doc_id = $1",
    )
    .bind(doc_id)
    .fetch_all(&self.pool)
    .await
    .map_err(|e| {
        NanoServiceError::new(
            format!("Failed to list members: {}", e),
            NanoServiceErrorStatus::InternalServerError,
        )
    })?;
    Ok(rows)
}

#[impl_transaction(SqlxPostGresDescriptor, GetDocumentMember, get_document_member)]
async fn get_document_member(
    &self,
    doc_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<Option<DocumentMember>, NanoServiceError> {
    let row = sqlx::query_as::<_, DocumentMember>(
        "SELECT doc_id, user_id, role FROM document_members WHERE doc_id = $1 AND user_id = $2",
    )
    .bind(doc_id)
    .bind(user_id)
    .fetch_optional(&self.pool)
    .await
    .map_err(|e| {
        NanoServiceError::new(
            format!("Failed to get member: {}", e),
            NanoServiceErrorStatus::InternalServerError,
        )
    })?;
    Ok(row)
}

#[impl_transaction(SqlxPostGresDescriptor, DeleteDocumentMember, delete_document_member)]
async fn delete_document_member(
    &self,
    doc_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<(), NanoServiceError> {
    utils::safe_eject!(
        sqlx::query("DELETE FROM document_members WHERE doc_id = $1 AND user_id = $2")
            .bind(doc_id)
            .bind(user_id)
            .execute(&self.pool)
            .await,
        NanoServiceErrorStatus::InternalServerError,
        "Failed to delete member"
    )?;
    Ok(())
}

#[impl_transaction(
    SqlxPostGresDescriptor,
    UpdateDocumentMemberRole,
    update_document_member_role
)]
async fn update_document_member_role(
    &self,
    doc_id: uuid::Uuid,
    user_id: uuid::Uuid,
    role: MemberRole,
) -> Result<DocumentMember, NanoServiceError> {
    let row = sqlx::query_as::<_, DocumentMember>(
        "UPDATE document_members SET role = $3 WHERE doc_id = $1 AND user_id = $2
         RETURNING doc_id, user_id, role",
    )
    .bind(doc_id)
    .bind(user_id)
    .bind(role)
    .fetch_optional(&self.pool)
    .await
    .map_err(|e| {
        NanoServiceError::new(
            format!("Failed to update member role: {}", e),
            NanoServiceErrorStatus::InternalServerError,
        )
    })?
    .ok_or_else(|| NanoServiceError::new("Member not found", NanoServiceErrorStatus::NotFound))?;
    Ok(row)
}
