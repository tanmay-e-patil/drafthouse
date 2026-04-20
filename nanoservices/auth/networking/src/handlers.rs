use actix_web::{HttpRequest, HttpResponse, cookie::Cookie, web};
use dal::postgres_txs::SqlxPostGresDescriptor;
use kernel::{LoginRequest, RegisterRequest, ResendVerificationRequest, VerifyEmailRequest};
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

type DalData = web::Data<SqlxPostGresDescriptor>;

const REFRESH_COOKIE: &str = "refresh_token";

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

fn refresh_cookie_max_age() -> i64 {
    std::env::var("REFRESH_TOKEN_EXPIRY_DAYS")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(30)
        * 24
        * 3600
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

pub async fn login(
    req: HttpRequest,
    body: web::Json<LoginRequest>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let (resp, raw_refresh) =
        auth_core::login::login_user(dal, &body.email, &body.password).await?;

    let cookie = Cookie::build(REFRESH_COOKIE, raw_refresh)
        .http_only(true)
        .same_site(actix_web::cookie::SameSite::Strict)
        .max_age(actix_web::cookie::time::Duration::seconds(
            refresh_cookie_max_age(),
        ))
        .path("/")
        .finish();

    Ok(HttpResponse::Ok().cookie(cookie).json(resp))
}

pub async fn refresh(req: HttpRequest) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;

    let raw_token = req
        .cookie(REFRESH_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or_else(|| {
            NanoServiceError::new(
                "Missing refresh token",
                NanoServiceErrorStatus::Unauthorized,
            )
        })?;

    let (resp, new_raw) = auth_core::login::refresh_access_token(dal, &raw_token).await?;

    let cookie = Cookie::build(REFRESH_COOKIE, new_raw)
        .http_only(true)
        .same_site(actix_web::cookie::SameSite::Strict)
        .max_age(actix_web::cookie::time::Duration::seconds(
            refresh_cookie_max_age(),
        ))
        .path("/")
        .finish();

    Ok(HttpResponse::Ok().cookie(cookie).json(resp))
}

pub async fn logout(req: HttpRequest) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;

    if let Some(cookie) = req.cookie(REFRESH_COOKIE) {
        auth_core::login::logout(dal, cookie.value()).await?;
    }

    let expired_cookie = Cookie::build(REFRESH_COOKIE, "")
        .http_only(true)
        .same_site(actix_web::cookie::SameSite::Strict)
        .max_age(actix_web::cookie::time::Duration::seconds(0))
        .path("/")
        .finish();

    Ok(HttpResponse::Ok().cookie(expired_cookie).finish())
}

pub async fn logout_all(req: HttpRequest) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    auth_core::login::logout_all(dal, claims.sub).await?;

    let expired_cookie = Cookie::build(REFRESH_COOKIE, "")
        .http_only(true)
        .same_site(actix_web::cookie::SameSite::Strict)
        .max_age(actix_web::cookie::time::Duration::seconds(0))
        .path("/")
        .finish();

    Ok(HttpResponse::Ok().cookie(expired_cookie).finish())
}
