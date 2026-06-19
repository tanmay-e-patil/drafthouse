pub mod welcome;

use dal::{
    AcceptInviteLink, CountDocumentsByOwner, CreateDocument, CreateInviteLink, DeleteDocument,
    DeleteDocumentMember, GetDocumentById, GetDocumentContent, GetDocumentMember,
    GetInviteLinkByToken, ListActiveInviteLinks, ListDocumentMembers, ListDocumentsByOwner,
    RevokeInviteLink, UpdateDocument, UpdateDocumentContent, UpdateDocumentMemberRole,
};
use kernel::{
    CreateInviteLinkRequest, Document, DocumentContentResponse, DocumentListResponse,
    DocumentMember, InviteLink, MemberRole, NewDocument, NewInviteLink, TitleUpdated,
    UpdateDocumentContentRequest, UpdateDocumentRequest, UpdateMemberRoleRequest, WsTicketResponse,
};
use nan_serve_publish_event::publish_event;
use rand::Rng;
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

/// Shared event runtime — re-exported from utils so all crates in this binary
/// share the same static handler registry.
pub mod tokio_event_adapter_runtime {
    pub use utils::event_runtime::*;
}

const DEFAULT_PAGE_LIMIT: i64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentAccessRole {
    Owner,
    Editor,
    Viewer,
    PublicViewer,
}

impl DocumentAccessRole {
    pub fn as_response_role(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Editor => "editor",
            Self::Viewer | Self::PublicViewer => "viewer",
        }
    }

    fn can_edit(self) -> bool {
        matches!(self, Self::Owner | Self::Editor)
    }
}

pub async fn resolve_document_access<D>(
    dal: &D,
    doc_id: uuid::Uuid,
    user_id: Option<uuid::Uuid>,
) -> Result<(Document, DocumentAccessRole), NanoServiceError>
where
    D: GetDocumentById + GetDocumentMember,
{
    let doc = dal.get_document_by_id(doc_id).await?.ok_or_else(|| {
        NanoServiceError::new("Document not found", NanoServiceErrorStatus::NotFound)
    })?;

    if let Some(user_id) = user_id {
        if doc.owner_id == user_id {
            return Ok((doc, DocumentAccessRole::Owner));
        }

        if let Some(member) = dal.get_document_member(doc_id, user_id).await? {
            let role = match member.role {
                MemberRole::Editor => DocumentAccessRole::Editor,
                MemberRole::Viewer => DocumentAccessRole::Viewer,
            };
            return Ok((doc, role));
        }
    }

    if doc.is_public {
        return Ok((doc, DocumentAccessRole::PublicViewer));
    }

    let (message, status) = if user_id.is_some() {
        (
            "You do not have access to this document",
            NanoServiceErrorStatus::Forbidden,
        )
    } else {
        (
            "Authentication required to access this document",
            NanoServiceErrorStatus::Unauthorized,
        )
    };

    Err(NanoServiceError::new(message, status))
}

pub async fn ensure_document_editor_access<D>(
    dal: &D,
    doc_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<Document, NanoServiceError>
where
    D: GetDocumentById + GetDocumentMember,
{
    let (doc, role) = resolve_document_access(dal, doc_id, Some(user_id)).await?;

    if role.can_edit() {
        Ok(doc)
    } else {
        Err(NanoServiceError::new(
            "Only editors can update this document",
            NanoServiceErrorStatus::Forbidden,
        ))
    }
}

pub async fn ensure_document_access<D>(
    dal: &D,
    doc_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<Document, NanoServiceError>
where
    D: GetDocumentById + GetDocumentMember,
{
    let doc = dal.get_document_by_id(doc_id).await?.ok_or_else(|| {
        NanoServiceError::new("Document not found", NanoServiceErrorStatus::NotFound)
    })?;

    if doc.owner_id == user_id {
        return Ok(doc);
    }

    let member = dal.get_document_member(doc_id, user_id).await?;
    if member.is_some() {
        Ok(doc)
    } else {
        Err(NanoServiceError::new(
            "You do not have access to this document",
            NanoServiceErrorStatus::Forbidden,
        ))
    }
}

pub async fn create_document<D>(
    dal: &D,
    owner_id: uuid::Uuid,
    title: &str,
) -> Result<Document, NanoServiceError>
where
    D: CreateDocument,
{
    let trimmed = title.trim().to_string();
    let title = if trimmed.is_empty() {
        "Untitled".to_string()
    } else {
        trimmed
    };

    let doc = dal.create_document(NewDocument { owner_id, title }).await?;
    tracing::info!(doc_id = %doc.id, owner_id = %owner_id, "document created");
    Ok(doc)
}

pub async fn get_document<D>(dal: &D, id: uuid::Uuid) -> Result<Document, NanoServiceError>
where
    D: GetDocumentById,
{
    dal.get_document_by_id(id).await?.ok_or_else(|| {
        NanoServiceError::new("Document not found", NanoServiceErrorStatus::NotFound)
    })
}

pub async fn update_document<D>(
    dal: &D,
    id: uuid::Uuid,
    owner_id: uuid::Uuid,
    request: &UpdateDocumentRequest,
) -> Result<Document, NanoServiceError>
where
    D: GetDocumentById + UpdateDocument,
{
    let existing = dal.get_document_by_id(id).await?.ok_or_else(|| {
        NanoServiceError::new("Document not found", NanoServiceErrorStatus::NotFound)
    })?;

    if existing.owner_id != owner_id {
        return Err(NanoServiceError::new(
            "Only the document owner can update it",
            NanoServiceErrorStatus::Forbidden,
        ));
    }

    let title = request.title.as_ref().map(|t| t.trim().to_string());
    let updated = dal.update_document(id, title, request.is_public).await?;

    if request.title.is_some() {
        let event = TitleUpdated {
            doc_id: updated.id,
            title: updated.title.clone(),
        };
        publish_event!(event);
    }

    Ok(updated)
}

pub async fn delete_document<D>(
    dal: &D,
    id: uuid::Uuid,
    owner_id: uuid::Uuid,
) -> Result<(), NanoServiceError>
where
    D: GetDocumentById + DeleteDocument,
{
    let existing = dal.get_document_by_id(id).await?.ok_or_else(|| {
        NanoServiceError::new("Document not found", NanoServiceErrorStatus::NotFound)
    })?;

    if existing.owner_id != owner_id {
        return Err(NanoServiceError::new(
            "Only the document owner can delete it",
            NanoServiceErrorStatus::Forbidden,
        ));
    }

    dal.delete_document(id).await?;
    tracing::info!(doc_id = %id, owner_id = %owner_id, "document deleted");
    Ok(())
}

pub async fn list_documents<D>(
    dal: &D,
    owner_id: uuid::Uuid,
    cursor: Option<uuid::Uuid>,
    limit: Option<i64>,
) -> Result<DocumentListResponse, NanoServiceError>
where
    D: ListDocumentsByOwner + CountDocumentsByOwner,
{
    let effective_limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT).min(100);
    let docs = dal
        .list_documents_by_owner(owner_id, cursor, effective_limit + 1)
        .await?;

    let has_more = docs.len() as i64 > effective_limit;
    let data: Vec<Document> = if has_more {
        docs.into_iter().take(effective_limit as usize).collect()
    } else {
        docs
    };

    let next_cursor = if has_more {
        data.last().map(|d| d.id)
    } else {
        None
    };

    Ok(DocumentListResponse {
        data,
        next_cursor,
        has_more,
    })
}

pub async fn get_document_content<D>(
    dal: &D,
    id: uuid::Uuid,
    user_id: Option<uuid::Uuid>,
) -> Result<DocumentContentResponse, NanoServiceError>
where
    D: GetDocumentById + GetDocumentMember + GetDocumentContent,
{
    resolve_document_access(dal, id, user_id).await?;

    let content = dal.get_document_content(id).await?.unwrap_or_default();
    Ok(DocumentContentResponse { content })
}

pub async fn update_document_content<D>(
    dal: &D,
    id: uuid::Uuid,
    user_id: uuid::Uuid,
    request: &UpdateDocumentContentRequest,
) -> Result<(), NanoServiceError>
where
    D: GetDocumentById + GetDocumentMember + UpdateDocumentContent,
{
    ensure_document_editor_access(dal, id, user_id).await?;

    dal.update_document_content(id, request.content.clone())
        .await
}

pub async fn issue_ws_ticket<D>(
    dal: &D,
    doc_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<WsTicketResponse, NanoServiceError>
where
    D: GetDocumentById + GetDocumentMember,
{
    let (_, role) = resolve_document_access(dal, doc_id, Some(user_id)).await?;
    let readonly = !role.can_edit();
    let ticket = auth_core::ws_capability::create_ws_capability(user_id, doc_id, readonly)?;

    Ok(WsTicketResponse { ticket })
}

const INVITE_TOKEN_LEN: usize = 32;

fn generate_invite_token() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(INVITE_TOKEN_LEN)
        .map(char::from)
        .collect()
}

pub async fn create_invite_link<D>(
    dal: &D,
    doc_id: uuid::Uuid,
    owner_id: uuid::Uuid,
    request: &CreateInviteLinkRequest,
) -> Result<InviteLink, NanoServiceError>
where
    D: GetDocumentById + CreateInviteLink,
{
    let doc = dal.get_document_by_id(doc_id).await?.ok_or_else(|| {
        NanoServiceError::new("Document not found", NanoServiceErrorStatus::NotFound)
    })?;

    if doc.owner_id != owner_id {
        return Err(NanoServiceError::new(
            "Only the document owner can create invite links",
            NanoServiceErrorStatus::Forbidden,
        ));
    }

    let token = generate_invite_token();
    let link = dal
        .create_invite_link(NewInviteLink {
            token,
            doc_id,
            role: request.role,
            created_by: owner_id,
            max_uses: request.max_uses,
            expires_at: request.expires_at,
        })
        .await?;

    tracing::info!(doc_id = %doc_id, role = ?request.role, "invite link created");
    Ok(link)
}

pub async fn list_invite_links<D>(
    dal: &D,
    doc_id: uuid::Uuid,
    owner_id: uuid::Uuid,
) -> Result<Vec<InviteLink>, NanoServiceError>
where
    D: GetDocumentById + ListActiveInviteLinks,
{
    let doc = dal.get_document_by_id(doc_id).await?.ok_or_else(|| {
        NanoServiceError::new("Document not found", NanoServiceErrorStatus::NotFound)
    })?;

    if doc.owner_id != owner_id {
        return Err(NanoServiceError::new(
            "Only the document owner can list invite links",
            NanoServiceErrorStatus::Forbidden,
        ));
    }

    dal.list_active_invite_links(doc_id).await
}

pub async fn revoke_invite_link<D>(
    dal: &D,
    doc_id: uuid::Uuid,
    owner_id: uuid::Uuid,
    token: &str,
) -> Result<(), NanoServiceError>
where
    D: GetDocumentById + GetInviteLinkByToken + RevokeInviteLink,
{
    let doc = dal.get_document_by_id(doc_id).await?.ok_or_else(|| {
        NanoServiceError::new("Document not found", NanoServiceErrorStatus::NotFound)
    })?;

    if doc.owner_id != owner_id {
        return Err(NanoServiceError::new(
            "Only the document owner can revoke invite links",
            NanoServiceErrorStatus::Forbidden,
        ));
    }

    let link = dal
        .get_invite_link_by_token(token.to_string())
        .await?
        .ok_or_else(|| {
            NanoServiceError::new("Invite link not found", NanoServiceErrorStatus::NotFound)
        })?;

    if link.doc_id != doc_id {
        return Err(NanoServiceError::new(
            "Invite link does not belong to this document",
            NanoServiceErrorStatus::NotFound,
        ));
    }

    dal.revoke_invite_link(token.to_string()).await
}

pub async fn accept_invite<D>(
    dal: &D,
    token: &str,
    user_id: uuid::Uuid,
) -> Result<DocumentMember, NanoServiceError>
where
    D: AcceptInviteLink,
{
    let member = dal.accept_invite_link(token.to_string(), user_id).await?;
    tracing::info!(doc_id = %member.doc_id, user_id = %user_id, role = ?member.role, "invite accepted");
    Ok(member)
}

pub async fn list_members<D>(
    dal: &D,
    doc_id: uuid::Uuid,
    owner_id: uuid::Uuid,
) -> Result<Vec<DocumentMember>, NanoServiceError>
where
    D: GetDocumentById + ListDocumentMembers,
{
    let doc = dal.get_document_by_id(doc_id).await?.ok_or_else(|| {
        NanoServiceError::new("Document not found", NanoServiceErrorStatus::NotFound)
    })?;

    if doc.owner_id != owner_id {
        return Err(NanoServiceError::new(
            "Only the document owner can list members",
            NanoServiceErrorStatus::Forbidden,
        ));
    }

    dal.list_document_members(doc_id).await
}

pub async fn remove_member<D>(
    dal: &D,
    doc_id: uuid::Uuid,
    owner_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<(), NanoServiceError>
where
    D: GetDocumentById + DeleteDocumentMember,
{
    let doc = dal.get_document_by_id(doc_id).await?.ok_or_else(|| {
        NanoServiceError::new("Document not found", NanoServiceErrorStatus::NotFound)
    })?;

    if doc.owner_id != owner_id {
        return Err(NanoServiceError::new(
            "Only the document owner can remove members",
            NanoServiceErrorStatus::Forbidden,
        ));
    }

    if user_id == owner_id {
        return Err(NanoServiceError::new(
            "Owner cannot remove themselves",
            NanoServiceErrorStatus::BadRequest,
        ));
    }

    dal.delete_document_member(doc_id, user_id).await
}

pub async fn update_member_role<D>(
    dal: &D,
    doc_id: uuid::Uuid,
    owner_id: uuid::Uuid,
    user_id: uuid::Uuid,
    request: &UpdateMemberRoleRequest,
) -> Result<DocumentMember, NanoServiceError>
where
    D: GetDocumentById + UpdateDocumentMemberRole,
{
    let doc = dal.get_document_by_id(doc_id).await?.ok_or_else(|| {
        NanoServiceError::new("Document not found", NanoServiceErrorStatus::NotFound)
    })?;

    if doc.owner_id != owner_id {
        return Err(NanoServiceError::new(
            "Only the document owner can update member roles",
            NanoServiceErrorStatus::Forbidden,
        ));
    }

    dal.update_document_member_role(doc_id, user_id, request.role)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    #[allow(unused_imports)]
    use kernel::TitleUpdated;
    use kernel::{DocumentMember, InviteLink, MemberRole};
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    fn test_document(owner_id: Uuid) -> Document {
        Document {
            id: Uuid::new_v4(),
            owner_id,
            title: "Test Document".to_string(),
            is_public: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    struct MockDal {
        documents: Arc<Mutex<Vec<Document>>>,
        next_id: Arc<Mutex<Uuid>>,
        content: Arc<Mutex<std::collections::HashMap<Uuid, String>>>,
        invite_links: Arc<Mutex<Vec<InviteLink>>>,
        members: Arc<Mutex<Vec<DocumentMember>>>,
    }

    impl MockDal {
        fn new() -> Self {
            Self {
                documents: Arc::new(Mutex::new(vec![])),
                next_id: Arc::new(Mutex::new(Uuid::new_v4())),
                content: Arc::new(Mutex::new(std::collections::HashMap::new())),
                invite_links: Arc::new(Mutex::new(vec![])),
                members: Arc::new(Mutex::new(vec![])),
            }
        }

        fn with_document(doc: Document) -> Self {
            Self {
                documents: Arc::new(Mutex::new(vec![doc])),
                next_id: Arc::new(Mutex::new(Uuid::new_v4())),
                content: Arc::new(Mutex::new(std::collections::HashMap::new())),
                invite_links: Arc::new(Mutex::new(vec![])),
                members: Arc::new(Mutex::new(vec![])),
            }
        }

        fn with_documents(docs: Vec<Document>) -> Self {
            Self {
                documents: Arc::new(Mutex::new(docs)),
                next_id: Arc::new(Mutex::new(Uuid::new_v4())),
                content: Arc::new(Mutex::new(std::collections::HashMap::new())),
                invite_links: Arc::new(Mutex::new(vec![])),
                members: Arc::new(Mutex::new(vec![])),
            }
        }

        fn with_document_and_content(doc: Document, content: &str) -> Self {
            let mut map = std::collections::HashMap::new();
            map.insert(doc.id, content.to_string());
            Self {
                documents: Arc::new(Mutex::new(vec![doc])),
                next_id: Arc::new(Mutex::new(Uuid::new_v4())),
                content: Arc::new(Mutex::new(map)),
                invite_links: Arc::new(Mutex::new(vec![])),
                members: Arc::new(Mutex::new(vec![])),
            }
        }

        fn with_document_and_link(doc: Document, link: InviteLink) -> Self {
            Self {
                documents: Arc::new(Mutex::new(vec![doc])),
                next_id: Arc::new(Mutex::new(Uuid::new_v4())),
                content: Arc::new(Mutex::new(std::collections::HashMap::new())),
                invite_links: Arc::new(Mutex::new(vec![link])),
                members: Arc::new(Mutex::new(vec![])),
            }
        }

        fn with_document_and_member(doc: Document, member: DocumentMember) -> Self {
            Self {
                documents: Arc::new(Mutex::new(vec![doc])),
                next_id: Arc::new(Mutex::new(Uuid::new_v4())),
                content: Arc::new(Mutex::new(std::collections::HashMap::new())),
                invite_links: Arc::new(Mutex::new(vec![])),
                members: Arc::new(Mutex::new(vec![member])),
            }
        }
    }

    fn test_invite_link(
        token: &str,
        doc_id: Uuid,
        created_by: Uuid,
        role: MemberRole,
        max_uses: Option<i32>,
        expires_at: Option<chrono::DateTime<Utc>>,
    ) -> InviteLink {
        InviteLink {
            token: token.to_string(),
            doc_id,
            role,
            created_by,
            max_uses,
            use_count: 0,
            expires_at,
            revoked_at: None,
        }
    }

    impl AcceptInviteLink for MockDal {
        fn accept_invite_link(
            &self,
            token: String,
            user_id: Uuid,
        ) -> impl std::future::Future<Output = Result<DocumentMember, NanoServiceError>> + Send
        {
            let links = Arc::clone(&self.invite_links);
            let members = Arc::clone(&self.members);
            async move {
                let mut links = links.lock().unwrap();
                let link = links.iter_mut().find(|l| l.token == token).ok_or_else(|| {
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

                link.use_count += 1;
                let member = DocumentMember {
                    doc_id: link.doc_id,
                    user_id,
                    email: None,
                    role: link.role,
                };
                members.lock().unwrap().push(member.clone());
                Ok(member)
            }
        }
    }

    impl CreateInviteLink for MockDal {
        fn create_invite_link(
            &self,
            new_link: NewInviteLink,
        ) -> impl std::future::Future<Output = Result<InviteLink, NanoServiceError>> + Send
        {
            let links = Arc::clone(&self.invite_links);
            async move {
                let link = InviteLink {
                    token: new_link.token,
                    doc_id: new_link.doc_id,
                    role: new_link.role,
                    created_by: new_link.created_by,
                    max_uses: new_link.max_uses,
                    use_count: 0,
                    expires_at: new_link.expires_at,
                    revoked_at: None,
                };
                links.lock().unwrap().push(link.clone());
                Ok(link)
            }
        }
    }

    impl GetInviteLinkByToken for MockDal {
        fn get_invite_link_by_token(
            &self,
            token: String,
        ) -> impl std::future::Future<Output = Result<Option<InviteLink>, NanoServiceError>> + Send
        {
            let links = Arc::clone(&self.invite_links);
            async move {
                Ok(links
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|l| l.token == token)
                    .cloned())
            }
        }
    }

    impl ListActiveInviteLinks for MockDal {
        fn list_active_invite_links(
            &self,
            doc_id: Uuid,
        ) -> impl std::future::Future<Output = Result<Vec<InviteLink>, NanoServiceError>> + Send
        {
            let links = Arc::clone(&self.invite_links);
            async move {
                let now = Utc::now();
                Ok(links
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|l| {
                        l.doc_id == doc_id
                            && l.revoked_at.is_none()
                            && l.expires_at.is_none_or(|e| e > now)
                    })
                    .cloned()
                    .collect())
            }
        }
    }

    impl RevokeInviteLink for MockDal {
        fn revoke_invite_link(
            &self,
            token: String,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let links = Arc::clone(&self.invite_links);
            async move {
                let mut links = links.lock().unwrap();
                if let Some(link) = links.iter_mut().find(|l| l.token == token) {
                    link.revoked_at = Some(Utc::now());
                }
                Ok(())
            }
        }
    }

    impl ListDocumentMembers for MockDal {
        fn list_document_members(
            &self,
            doc_id: Uuid,
        ) -> impl std::future::Future<Output = Result<Vec<DocumentMember>, NanoServiceError>> + Send
        {
            let members = Arc::clone(&self.members);
            async move {
                Ok(members
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|m| m.doc_id == doc_id)
                    .cloned()
                    .collect())
            }
        }
    }

    impl GetDocumentMember for MockDal {
        fn get_document_member(
            &self,
            doc_id: Uuid,
            user_id: Uuid,
        ) -> impl std::future::Future<Output = Result<Option<DocumentMember>, NanoServiceError>> + Send
        {
            let members = Arc::clone(&self.members);
            async move {
                Ok(members
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|m| m.doc_id == doc_id && m.user_id == user_id)
                    .cloned())
            }
        }
    }

    impl DeleteDocumentMember for MockDal {
        fn delete_document_member(
            &self,
            doc_id: Uuid,
            user_id: Uuid,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let members = Arc::clone(&self.members);
            async move {
                members
                    .lock()
                    .unwrap()
                    .retain(|m| !(m.doc_id == doc_id && m.user_id == user_id));
                Ok(())
            }
        }
    }

    impl UpdateDocumentMemberRole for MockDal {
        fn update_document_member_role(
            &self,
            doc_id: Uuid,
            user_id: Uuid,
            role: MemberRole,
        ) -> impl std::future::Future<Output = Result<DocumentMember, NanoServiceError>> + Send
        {
            let members = Arc::clone(&self.members);
            async move {
                let mut members = members.lock().unwrap();
                if let Some(m) = members
                    .iter_mut()
                    .find(|m| m.doc_id == doc_id && m.user_id == user_id)
                {
                    m.role = role;
                    return Ok(m.clone());
                }
                Err(NanoServiceError::new(
                    "Member not found",
                    NanoServiceErrorStatus::NotFound,
                ))
            }
        }
    }

    impl CreateDocument for MockDal {
        fn create_document(
            &self,
            new_doc: NewDocument,
        ) -> impl std::future::Future<Output = Result<Document, NanoServiceError>> + Send {
            let docs = Arc::clone(&self.documents);
            let id = *self.next_id.lock().unwrap();
            async move {
                let doc = Document {
                    id,
                    owner_id: new_doc.owner_id,
                    title: new_doc.title,
                    is_public: false,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                docs.lock().unwrap().push(doc.clone());
                Ok(doc)
            }
        }
    }

    impl GetDocumentById for MockDal {
        fn get_document_by_id(
            &self,
            id: Uuid,
        ) -> impl std::future::Future<Output = Result<Option<Document>, NanoServiceError>> + Send
        {
            let docs = Arc::clone(&self.documents);
            async move { Ok(docs.lock().unwrap().iter().find(|d| d.id == id).cloned()) }
        }
    }

    impl UpdateDocument for MockDal {
        fn update_document(
            &self,
            id: Uuid,
            title: Option<String>,
            is_public: Option<bool>,
        ) -> impl std::future::Future<Output = Result<Document, NanoServiceError>> + Send {
            let docs = Arc::clone(&self.documents);
            async move {
                let mut docs = docs.lock().unwrap();
                if let Some(doc) = docs.iter_mut().find(|d| d.id == id) {
                    if let Some(t) = title {
                        doc.title = t;
                    }
                    if let Some(p) = is_public {
                        doc.is_public = p;
                    }
                    doc.updated_at = Utc::now();
                    return Ok(doc.clone());
                }
                Err(NanoServiceError::new(
                    "Document not found",
                    NanoServiceErrorStatus::NotFound,
                ))
            }
        }
    }

    impl DeleteDocument for MockDal {
        fn delete_document(
            &self,
            id: Uuid,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let docs = Arc::clone(&self.documents);
            async move {
                docs.lock().unwrap().retain(|d| d.id != id);
                Ok(())
            }
        }
    }

    impl ListDocumentsByOwner for MockDal {
        fn list_documents_by_owner(
            &self,
            owner_id: Uuid,
            cursor: Option<Uuid>,
            limit: i64,
        ) -> impl std::future::Future<Output = Result<Vec<Document>, NanoServiceError>> + Send
        {
            let docs = Arc::clone(&self.documents);
            async move {
                let all = docs.lock().unwrap();
                let owned: Vec<Document> = all
                    .iter()
                    .filter(|d| d.owner_id == owner_id)
                    .cloned()
                    .collect();

                let start_idx = if let Some(cursor_id) = cursor {
                    owned
                        .iter()
                        .position(|d| d.id == cursor_id)
                        .map(|i| i + 1)
                        .unwrap_or(owned.len())
                } else {
                    0
                };

                Ok(owned
                    .into_iter()
                    .skip(start_idx)
                    .take(limit as usize)
                    .collect())
            }
        }
    }

    impl CountDocumentsByOwner for MockDal {
        fn count_documents_by_owner(
            &self,
            owner_id: Uuid,
        ) -> impl std::future::Future<Output = Result<i64, NanoServiceError>> + Send {
            let docs = Arc::clone(&self.documents);
            async move {
                Ok(docs
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|d| d.owner_id == owner_id)
                    .count() as i64)
            }
        }
    }

    impl GetDocumentContent for MockDal {
        fn get_document_content(
            &self,
            id: Uuid,
        ) -> impl std::future::Future<Output = Result<Option<String>, NanoServiceError>> + Send
        {
            let content = Arc::clone(&self.content);
            async move { Ok(content.lock().unwrap().get(&id).cloned()) }
        }
    }

    impl UpdateDocumentContent for MockDal {
        fn update_document_content(
            &self,
            id: Uuid,
            content: String,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let content_map = Arc::clone(&self.content);
            async move {
                content_map.lock().unwrap().insert(id, content);
                Ok(())
            }
        }
    }

    // Needed in tests that register test-only event handlers
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool, Ordering};

    // ── create_document ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn create_document_stores_and_returns() {
        let dal = MockDal::new();
        let owner_id = Uuid::new_v4();
        let result = create_document(&dal, owner_id, "My Doc").await;
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert_eq!(doc.title, "My Doc");
        assert_eq!(doc.owner_id, owner_id);
        assert_eq!(dal.documents.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn create_document_trims_title() {
        let dal = MockDal::new();
        let result = create_document(&dal, Uuid::new_v4(), "  Spaced  ").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().title, "Spaced");
    }

    #[tokio::test]
    async fn create_document_empty_title_defaults_to_untitled() {
        let dal = MockDal::new();
        let result = create_document(&dal, Uuid::new_v4(), "  ").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().title, "Untitled");
    }

    // ── get_document ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn get_document_returns_existing() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = get_document(&dal, doc.id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, doc.id);
    }

    #[tokio::test]
    async fn get_document_not_found_returns_404() {
        let dal = MockDal::new();
        let result = get_document(&dal, Uuid::new_v4()).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, NanoServiceErrorStatus::NotFound);
    }

    // ── update_document + TitleUpdated event ──────────────────────────────────

    #[tokio::test]
    async fn update_document_publishes_title_updated_event() {
        static FIRED: AtomicBool = AtomicBool::new(false);

        fn handler(data: Vec<u8>) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async move {
                let event: TitleUpdated =
                    bincode::deserialize(&data).expect("deserialize TitleUpdated");
                assert_eq!(event.title, "Event Title");
                FIRED.store(true, Ordering::SeqCst);
            })
        }
        crate::tokio_event_adapter_runtime::insert_into_hashmap(
            "TitleUpdated".to_string(),
            handler,
        );

        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        update_document(
            &dal,
            doc.id,
            owner_id,
            &UpdateDocumentRequest {
                title: Some("Event Title".to_string()),
                is_public: None,
            },
        )
        .await
        .unwrap();

        tokio::task::yield_now().await;
        assert!(
            FIRED.load(Ordering::SeqCst),
            "TitleUpdated event not published"
        );
    }

    #[tokio::test]
    async fn update_document_no_event_when_title_not_in_request() {
        static FIRED2: AtomicBool = AtomicBool::new(false);

        fn handler2(data: Vec<u8>) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async move {
                let _: TitleUpdated = bincode::deserialize(&data).unwrap();
                FIRED2.store(true, Ordering::SeqCst);
            })
        }
        crate::tokio_event_adapter_runtime::insert_into_hashmap(
            "TitleUpdated".to_string(),
            handler2,
        );

        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        update_document(
            &dal,
            doc.id,
            owner_id,
            &UpdateDocumentRequest {
                title: None,
                is_public: Some(true),
            },
        )
        .await
        .unwrap();

        tokio::task::yield_now().await;
        assert!(
            !FIRED2.load(Ordering::SeqCst),
            "TitleUpdated must not fire when title not updated"
        );
    }

    #[tokio::test]
    async fn update_document_owner_succeeds() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = update_document(
            &dal,
            doc.id,
            owner_id,
            &UpdateDocumentRequest {
                title: Some("New Title".to_string()),
                is_public: None,
            },
        )
        .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().title, "New Title");
    }

    #[tokio::test]
    async fn update_document_non_owner_returns_403() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = update_document(
            &dal,
            doc.id,
            Uuid::new_v4(),
            &UpdateDocumentRequest {
                title: Some("Hacked".to_string()),
                is_public: None,
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::Forbidden
        );
    }

    #[tokio::test]
    async fn update_document_not_found_returns_404() {
        let dal = MockDal::new();
        let result = update_document(
            &dal,
            Uuid::new_v4(),
            Uuid::new_v4(),
            &UpdateDocumentRequest {
                title: Some("X".to_string()),
                is_public: None,
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, NanoServiceErrorStatus::NotFound);
    }

    // ── delete_document ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn delete_document_owner_succeeds() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = delete_document(&dal, doc.id, owner_id).await;
        assert!(result.is_ok());
        assert!(dal.documents.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_document_non_owner_returns_403() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = delete_document(&dal, doc.id, Uuid::new_v4()).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::Forbidden
        );
    }

    #[tokio::test]
    async fn delete_document_not_found_returns_404() {
        let dal = MockDal::new();
        let result = delete_document(&dal, Uuid::new_v4(), Uuid::new_v4()).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, NanoServiceErrorStatus::NotFound);
    }

    // ── list_documents ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_documents_empty_returns_empty_list() {
        let dal = MockDal::new();
        let result = list_documents(&dal, Uuid::new_v4(), None, None).await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert!(resp.data.is_empty());
        assert!(!resp.has_more);
        assert!(resp.next_cursor.is_none());
    }

    #[tokio::test]
    async fn list_documents_returns_owned_docs() {
        let owner_id = Uuid::new_v4();
        let docs = vec![
            test_document(owner_id),
            test_document(owner_id),
            test_document(Uuid::new_v4()),
        ];
        let dal = MockDal::with_documents(docs);
        let result = list_documents(&dal, owner_id, None, None).await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.data.len(), 2);
    }

    #[tokio::test]
    async fn list_documents_pagination_with_cursor() {
        let owner_id = Uuid::new_v4();
        let docs: Vec<Document> = (0..5).map(|_| test_document(owner_id)).collect();
        let dal = MockDal::with_documents(docs);

        let first_page = list_documents(&dal, owner_id, None, Some(3)).await.unwrap();
        assert_eq!(first_page.data.len(), 3);
        assert!(first_page.has_more);
        assert!(first_page.next_cursor.is_some());

        let second_page = list_documents(&dal, owner_id, first_page.next_cursor, Some(3))
            .await
            .unwrap();
        assert_eq!(second_page.data.len(), 2);
        assert!(!second_page.has_more);
    }

    // ── get_document_content ──────────────────────────────────────────────────

    #[tokio::test]
    async fn get_document_content_returns_content() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document_and_content(doc.clone(), "# Hello");
        let result = get_document_content(&dal, doc.id, Some(owner_id)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "# Hello");
    }

    #[tokio::test]
    async fn get_document_content_returns_empty_when_no_content() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = get_document_content(&dal, doc.id, Some(owner_id)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "");
    }

    #[tokio::test]
    async fn get_document_content_allows_public_unauthenticated_reader() {
        let owner_id = Uuid::new_v4();
        let mut doc = test_document(owner_id);
        doc.is_public = true;
        let dal = MockDal::with_document_and_content(doc.clone(), "# Public");
        let result = get_document_content(&dal, doc.id, None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "# Public");
    }

    #[tokio::test]
    async fn get_document_content_private_unauthenticated_returns_401() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = get_document_content(&dal, doc.id, None).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::Unauthorized
        );
    }

    #[tokio::test]
    async fn get_document_content_not_found_returns_404() {
        let dal = MockDal::new();
        let result = get_document_content(&dal, Uuid::new_v4(), Some(Uuid::new_v4())).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, NanoServiceErrorStatus::NotFound);
    }

    // ── update_document_content ───────────────────────────────────────────────

    #[tokio::test]
    async fn update_document_content_succeeds() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = update_document_content(
            &dal,
            doc.id,
            owner_id,
            &UpdateDocumentContentRequest {
                content: "# New Content".to_string(),
            },
        )
        .await;
        assert!(result.is_ok());
        let content = dal.content.lock().unwrap().get(&doc.id).cloned();
        assert_eq!(content, Some("# New Content".to_string()));
    }

    #[tokio::test]
    async fn update_document_content_viewer_returns_403() {
        let owner_id = Uuid::new_v4();
        let viewer_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let member = DocumentMember {
            doc_id: doc.id,
            user_id: viewer_id,
            email: None,
            role: MemberRole::Viewer,
        };
        let dal = MockDal::with_document_and_member(doc.clone(), member);
        let result = update_document_content(
            &dal,
            doc.id,
            viewer_id,
            &UpdateDocumentContentRequest {
                content: "# Viewer edit".to_string(),
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::Forbidden
        );
    }

    #[tokio::test]
    async fn update_document_content_editor_member_succeeds() {
        let owner_id = Uuid::new_v4();
        let editor_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let member = DocumentMember {
            doc_id: doc.id,
            user_id: editor_id,
            email: None,
            role: MemberRole::Editor,
        };
        let dal = MockDal::with_document_and_member(doc.clone(), member);
        let result = update_document_content(
            &dal,
            doc.id,
            editor_id,
            &UpdateDocumentContentRequest {
                content: "# Editor edit".to_string(),
            },
        )
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn update_document_content_not_found_returns_404() {
        let dal = MockDal::new();
        let result = update_document_content(
            &dal,
            Uuid::new_v4(),
            Uuid::new_v4(),
            &UpdateDocumentContentRequest {
                content: "content".to_string(),
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, NanoServiceErrorStatus::NotFound);
    }

    // ── issue_ws_ticket ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn issue_ws_ticket_returns_ticket_for_existing_doc() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = issue_ws_ticket(&dal, doc.id, owner_id).await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        let claims = auth_core::ws_capability::verify_ws_capability(&resp.ticket).unwrap();
        assert_eq!(claims.sub, owner_id);
        assert_eq!(claims.doc_id, doc.id);
        assert!(!claims.readonly);
    }

    #[tokio::test]
    async fn issue_ws_ticket_viewer_returns_ticket() {
        let owner_id = Uuid::new_v4();
        let viewer_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let member = DocumentMember {
            doc_id: doc.id,
            user_id: viewer_id,
            email: None,
            role: MemberRole::Viewer,
        };
        let dal = MockDal::with_document_and_member(doc.clone(), member);
        let result = issue_ws_ticket(&dal, doc.id, viewer_id).await;
        let resp = result.unwrap();
        let claims = auth_core::ws_capability::verify_ws_capability(&resp.ticket).unwrap();
        assert_eq!(claims.sub, viewer_id);
        assert_eq!(claims.doc_id, doc.id);
        assert!(claims.readonly);
    }

    #[tokio::test]
    async fn issue_ws_ticket_not_found_returns_404() {
        let dal = MockDal::new();
        let result = issue_ws_ticket(&dal, Uuid::new_v4(), Uuid::new_v4()).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, NanoServiceErrorStatus::NotFound);
    }

    // ── create_invite_link ────────────────────────────────────────────────────

    #[tokio::test]
    async fn create_invite_link_owner_succeeds() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = create_invite_link(
            &dal,
            doc.id,
            owner_id,
            &CreateInviteLinkRequest {
                role: MemberRole::Editor,
                expires_at: None,
                max_uses: None,
            },
        )
        .await;
        assert!(result.is_ok());
        let link = result.unwrap();
        assert_eq!(link.doc_id, doc.id);
        assert_eq!(link.role, MemberRole::Editor);
        assert_eq!(link.use_count, 0);
        assert_eq!(link.token.len(), 32);
    }

    #[tokio::test]
    async fn create_invite_link_non_owner_returns_403() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());
        let result = create_invite_link(
            &dal,
            doc.id,
            Uuid::new_v4(),
            &CreateInviteLinkRequest {
                role: MemberRole::Viewer,
                expires_at: None,
                max_uses: None,
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::Forbidden
        );
    }

    #[tokio::test]
    async fn create_invite_link_doc_not_found_returns_404() {
        let dal = MockDal::new();
        let result = create_invite_link(
            &dal,
            Uuid::new_v4(),
            Uuid::new_v4(),
            &CreateInviteLinkRequest {
                role: MemberRole::Viewer,
                expires_at: None,
                max_uses: None,
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, NanoServiceErrorStatus::NotFound);
    }

    // ── accept_invite ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn accept_invite_adds_member_with_correct_role() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let user_id = Uuid::new_v4();
        let link = test_invite_link("tok1", doc.id, owner_id, MemberRole::Editor, None, None);
        let dal = MockDal::with_document_and_link(doc.clone(), link);

        let result = accept_invite(&dal, "tok1", user_id).await;
        assert!(result.is_ok());
        let member = result.unwrap();
        assert_eq!(member.role, MemberRole::Editor);
        assert_eq!(member.user_id, user_id);
        assert_eq!(member.doc_id, doc.id);
    }

    #[tokio::test]
    async fn accept_invite_increments_use_count() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let link = test_invite_link("tok2", doc.id, owner_id, MemberRole::Viewer, Some(3), None);
        let dal = MockDal::with_document_and_link(doc, link);

        accept_invite(&dal, "tok2", Uuid::new_v4()).await.unwrap();
        let count = dal.invite_links.lock().unwrap()[0].use_count;
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn accept_invite_revoked_link_returns_gone() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let mut link = test_invite_link("tok3", doc.id, owner_id, MemberRole::Editor, None, None);
        link.revoked_at = Some(Utc::now());
        let dal = MockDal::with_document_and_link(doc, link);

        let result = accept_invite(&dal, "tok3", Uuid::new_v4()).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, NanoServiceErrorStatus::Gone);
    }

    #[tokio::test]
    async fn accept_invite_expired_link_returns_gone() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let mut link = test_invite_link("tok4", doc.id, owner_id, MemberRole::Editor, None, None);
        link.expires_at = Some(Utc::now() - chrono::Duration::seconds(1));
        let dal = MockDal::with_document_and_link(doc, link);

        let result = accept_invite(&dal, "tok4", Uuid::new_v4()).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, NanoServiceErrorStatus::Gone);
    }

    #[tokio::test]
    async fn accept_invite_exhausted_link_returns_gone() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let mut link =
            test_invite_link("tok5", doc.id, owner_id, MemberRole::Editor, Some(2), None);
        link.use_count = 2;
        let dal = MockDal::with_document_and_link(doc, link);

        let result = accept_invite(&dal, "tok5", Uuid::new_v4()).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, NanoServiceErrorStatus::Gone);
    }

    #[tokio::test]
    async fn accept_invite_not_found_returns_404() {
        let dal = MockDal::new();
        let result = accept_invite(&dal, "nonexistent", Uuid::new_v4()).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, NanoServiceErrorStatus::NotFound);
    }

    // ── list_invite_links ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_invite_links_returns_active_links_for_owner() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let link = test_invite_link("tok6", doc.id, owner_id, MemberRole::Viewer, None, None);
        let dal = MockDal::with_document_and_link(doc.clone(), link);

        let result = list_invite_links(&dal, doc.id, owner_id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn list_invite_links_non_owner_returns_403() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());

        let result = list_invite_links(&dal, doc.id, Uuid::new_v4()).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::Forbidden
        );
    }

    // ── revoke_invite_link ────────────────────────────────────────────────────

    #[tokio::test]
    async fn revoke_invite_link_owner_succeeds() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let link = test_invite_link("tok7", doc.id, owner_id, MemberRole::Editor, None, None);
        let dal = MockDal::with_document_and_link(doc.clone(), link);

        let result = revoke_invite_link(&dal, doc.id, owner_id, "tok7").await;
        assert!(result.is_ok());
        let revoked = dal.invite_links.lock().unwrap()[0].revoked_at;
        assert!(revoked.is_some());
    }

    #[tokio::test]
    async fn revoke_invite_link_non_owner_returns_403() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let link = test_invite_link("tok8", doc.id, owner_id, MemberRole::Editor, None, None);
        let dal = MockDal::with_document_and_link(doc.clone(), link);

        let result = revoke_invite_link(&dal, doc.id, Uuid::new_v4(), "tok8").await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::Forbidden
        );
    }

    // ── list_members ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_members_returns_members_for_owner() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let member = DocumentMember {
            doc_id: doc.id,
            user_id: Uuid::new_v4(),
            email: Some("member@example.com".to_string()),
            role: MemberRole::Editor,
        };
        let dal = MockDal::with_document_and_member(doc.clone(), member);

        let result = list_members(&dal, doc.id, owner_id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn list_members_non_owner_returns_403() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());

        let result = list_members(&dal, doc.id, Uuid::new_v4()).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::Forbidden
        );
    }

    // ── remove_member ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn remove_member_owner_removes_other_user() {
        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let member = DocumentMember {
            doc_id: doc.id,
            user_id,
            email: Some("member@example.com".to_string()),
            role: MemberRole::Editor,
        };
        let dal = MockDal::with_document_and_member(doc.clone(), member);

        let result = remove_member(&dal, doc.id, owner_id, user_id).await;
        assert!(result.is_ok());
        assert!(dal.members.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn remove_member_owner_cannot_remove_self() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());

        let result = remove_member(&dal, doc.id, owner_id, owner_id).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::BadRequest
        );
    }

    #[tokio::test]
    async fn remove_member_non_owner_returns_403() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());

        let result = remove_member(&dal, doc.id, Uuid::new_v4(), Uuid::new_v4()).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::Forbidden
        );
    }

    // ── update_member_role ────────────────────────────────────────────────────

    #[tokio::test]
    async fn update_member_role_owner_succeeds() {
        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let member = DocumentMember {
            doc_id: doc.id,
            user_id,
            email: Some("member@example.com".to_string()),
            role: MemberRole::Editor,
        };
        let dal = MockDal::with_document_and_member(doc.clone(), member);

        let result = update_member_role(
            &dal,
            doc.id,
            owner_id,
            user_id,
            &UpdateMemberRoleRequest {
                role: MemberRole::Viewer,
            },
        )
        .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().role, MemberRole::Viewer);
    }

    #[tokio::test]
    async fn update_member_role_non_owner_returns_403() {
        let owner_id = Uuid::new_v4();
        let doc = test_document(owner_id);
        let dal = MockDal::with_document(doc.clone());

        let result = update_member_role(
            &dal,
            doc.id,
            Uuid::new_v4(),
            Uuid::new_v4(),
            &UpdateMemberRoleRequest {
                role: MemberRole::Viewer,
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::Forbidden
        );
    }
}
