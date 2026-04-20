use actix_web::{HttpRequest, HttpResponse, web};
use dal::postgres_txs::SqlxPostGresDescriptor;
use kernel::{RegisterRequest, ResendVerificationRequest, VerifyEmailRequest};
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

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

pub async fn register(
    req: HttpRequest,
    body: web::Json<RegisterRequest>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let result = auth_core::register::register_user(dal, &body.email, &body.password).await?;
    Ok(HttpResponse::Created().json(result))
}

pub async fn verify_email(
    req: HttpRequest,
    body: web::Json<VerifyEmailRequest>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let result = auth_core::verify::verify_email(dal, &body.token).await?;
    Ok(HttpResponse::Ok().json(result))
}

pub async fn resend_verification(
    req: HttpRequest,
    body: web::Json<ResendVerificationRequest>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let result = auth_core::resend::resend_verification(dal, &body.email).await?;
    Ok(HttpResponse::Ok().json(result))
}
