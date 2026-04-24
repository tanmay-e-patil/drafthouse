use actix_web::{HttpRequest, HttpResponse, web};
use collab_core::{DocStore, awareness_last_active_to_datetime};
use dal::postgres_txs::SqlxPostGresDescriptor;
use kernel::{
    CreateDocumentRequest, CreateInviteLinkRequest, DocumentPresencePeer, DocumentPresenceResponse,
    UpdateDocumentContentRequest, UpdateDocumentRequest, UpdateMemberRoleRequest,
};
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};
use uuid::Uuid;

type DalData = web::Data<SqlxPostGresDescriptor>;
type DocStoreData = web::Data<DocStore>;

fn get_dal(req: &HttpRequest) -> Result<&SqlxPostGresDescriptor, NanoServiceError> {
    req.app_data::<DalData>()
        .ok_or_else(|| {
            NanoServiceError::new(
                "Server misconfigured: DAL not available",
                NanoServiceErrorStatus::InternalServerError,
            )
        })
        .map(|d| d.get_ref())
}

fn get_doc_store(req: &HttpRequest) -> Result<&DocStore, NanoServiceError> {
    req.app_data::<DocStoreData>()
        .ok_or_else(|| {
            NanoServiceError::new(
                "Server misconfigured: DocStore not available",
                NanoServiceErrorStatus::InternalServerError,
            )
        })
        .map(|d| d.get_ref())
}

pub async fn create_document(
    req: HttpRequest,
    body: web::Json<CreateDocumentRequest>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    tracing::debug!(user_id = %claims.sub, "create_document");
    let doc = documents_core::create_document(
        dal,
        claims.sub,
        body.title.as_deref().unwrap_or("Untitled"),
    )
    .await?;
    Ok(HttpResponse::Created().json(doc))
}

pub async fn list_documents(
    req: HttpRequest,
    query: web::Query<ListDocumentsQuery>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    let cursor = query
        .cursor
        .as_ref()
        .map(|c| Uuid::parse_str(c))
        .transpose()
        .map_err(|_| {
            NanoServiceError::new("Invalid cursor format", NanoServiceErrorStatus::BadRequest)
        })?;
    let limit = query.limit.map(|l| l as i64);
    let result = documents_core::list_documents(dal, claims.sub, cursor, limit).await?;
    Ok(HttpResponse::Ok().json(result))
}

pub async fn get_document(
    req: HttpRequest,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let doc = documents_core::get_document(dal, *path).await?;
    Ok(HttpResponse::Ok().json(doc))
}

pub async fn update_document(
    req: HttpRequest,
    path: web::Path<Uuid>,
    body: web::Json<UpdateDocumentRequest>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    tracing::debug!(user_id = %claims.sub, "update_document");
    let doc = documents_core::update_document(dal, *path, claims.sub, &body).await?;
    Ok(HttpResponse::Ok().json(doc))
}

pub async fn delete_document(
    req: HttpRequest,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    tracing::debug!(user_id = %claims.sub, doc_id = %path, "delete_document");
    documents_core::delete_document(dal, *path, claims.sub).await?;
    Ok(HttpResponse::NoContent().finish())
}

pub async fn get_document_presence(
    req: HttpRequest,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let doc_store = get_doc_store(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    let doc_id = *path;

    documents_core::ensure_document_access(dal, doc_id, claims.sub).await?;

    let cutoff_ms = chrono::Utc::now().timestamp_millis() - 5 * 60_000;
    let data = doc_store
        .get(&doc_id)
        .map(|entry| {
            entry
                .awareness_peers()
                .into_iter()
                .filter(|peer| peer.last_active_ms > cutoff_ms)
                .filter_map(|peer| {
                    awareness_last_active_to_datetime(peer.last_active_ms).map(|last_active| {
                        DocumentPresencePeer {
                            name: peer.name,
                            color: peer.color,
                            last_active,
                        }
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(HttpResponse::Ok().json(DocumentPresenceResponse { data }))
}

#[derive(serde::Deserialize)]
pub struct ListDocumentsQuery {
    pub cursor: Option<String>,
    pub limit: Option<i32>,
}

pub async fn get_document_content(
    req: HttpRequest,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let result = documents_core::get_document_content(dal, *path).await?;
    Ok(HttpResponse::Ok().json(result))
}

pub async fn update_document_content(
    req: HttpRequest,
    path: web::Path<Uuid>,
    body: web::Json<UpdateDocumentContentRequest>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    documents_core::update_document_content(dal, *path, &body).await?;
    Ok(HttpResponse::Ok().finish())
}

pub async fn issue_ws_ticket(
    req: HttpRequest,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    tracing::debug!(user_id = %claims.sub, doc_id = %path, "issue_ws_ticket");
    let resp = documents_core::issue_ws_ticket(dal, *path, claims.sub).await?;
    Ok(HttpResponse::Created().json(resp))
}

pub async fn create_invite_link(
    req: HttpRequest,
    path: web::Path<Uuid>,
    body: web::Json<CreateInviteLinkRequest>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    let link = documents_core::create_invite_link(dal, *path, claims.sub, &body).await?;
    Ok(HttpResponse::Created().json(link))
}

pub async fn list_invite_links(
    req: HttpRequest,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    let links = documents_core::list_invite_links(dal, *path, claims.sub).await?;
    Ok(HttpResponse::Ok().json(links))
}

pub async fn revoke_invite_link(
    req: HttpRequest,
    path: web::Path<(Uuid, String)>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    let (doc_id, token) = path.into_inner();
    documents_core::revoke_invite_link(dal, doc_id, claims.sub, &token).await?;
    Ok(HttpResponse::NoContent().finish())
}

pub async fn accept_invite(
    req: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    tracing::debug!(user_id = %claims.sub, doc_id = %path, "accept_invite");
    let member = documents_core::accept_invite(dal, &path, claims.sub).await?;
    Ok(HttpResponse::Ok().json(member))
}

pub async fn list_members(
    req: HttpRequest,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    let members = documents_core::list_members(dal, *path, claims.sub).await?;
    Ok(HttpResponse::Ok().json(members))
}

pub async fn remove_member(
    req: HttpRequest,
    path: web::Path<(Uuid, Uuid)>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    let (doc_id, user_id) = path.into_inner();
    documents_core::remove_member(dal, doc_id, claims.sub, user_id).await?;
    Ok(HttpResponse::NoContent().finish())
}

pub async fn update_member_role(
    req: HttpRequest,
    path: web::Path<(Uuid, Uuid)>,
    body: web::Json<UpdateMemberRoleRequest>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    let (doc_id, user_id) = path.into_inner();
    let member =
        documents_core::update_member_role(dal, doc_id, claims.sub, user_id, &body).await?;
    Ok(HttpResponse::Ok().json(member))
}
