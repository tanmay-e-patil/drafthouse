use actix_web::{HttpRequest, HttpResponse, web};
use dal::postgres_txs::SqlxPostGresDescriptor;
use kernel::{CreateDocumentRequest, UpdateDocumentContentRequest, UpdateDocumentRequest};
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};
use uuid::Uuid;

type DalData = web::Data<SqlxPostGresDescriptor>;

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

pub async fn create_document(
    req: HttpRequest,
    body: web::Json<CreateDocumentRequest>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
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
    let doc = documents_core::update_document(dal, *path, claims.sub, &body).await?;
    Ok(HttpResponse::Ok().json(doc))
}

pub async fn delete_document(
    req: HttpRequest,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    documents_core::delete_document(dal, *path, claims.sub).await?;
    Ok(HttpResponse::NoContent().finish())
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
