use actix_web::{HttpRequest, HttpResponse, cookie::Cookie, web};
use dal::postgres_txs::SqlxPostGresDescriptor;
use kernel::{
    ChangePasswordRequest, DeleteAccountRequest, ForgotPasswordRequest, LoginRequest,
    RegisterRequest, ResendVerificationRequest, ResetPasswordRequest, VerifyEmailRequest,
};
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

fn expired_refresh_cookie() -> Cookie<'static> {
    Cookie::build(REFRESH_COOKIE, "")
        .http_only(true)
        .same_site(actix_web::cookie::SameSite::Strict)
        .max_age(actix_web::cookie::time::Duration::seconds(0))
        .path("/")
        .finish()
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
    let (mut resp, raw_refresh, user_id, is_first_login) =
        auth_core::login::login_user(dal, &body.email, &body.password).await?;

    if is_first_login {
        let doc = documents_core::welcome::create_welcome_document(dal, user_id).await?;
        resp.welcome_doc_id = Some(doc.id);
    }

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

    Ok(HttpResponse::Ok().cookie(expired_refresh_cookie()).finish())
}

pub async fn logout_all(req: HttpRequest) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    auth_core::login::logout_all(dal, claims.sub).await?;

    Ok(HttpResponse::Ok().cookie(expired_refresh_cookie()).finish())
}

pub async fn forgot_password(
    req: HttpRequest,
    body: web::Json<ForgotPasswordRequest>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let result = auth_core::password_reset::forgot_password(dal, &body.email).await?;
    Ok(HttpResponse::Ok().json(result))
}

pub async fn reset_password(
    req: HttpRequest,
    body: web::Json<ResetPasswordRequest>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let result =
        auth_core::password_reset::reset_password(dal, &body.token, &body.new_password).await?;
    Ok(HttpResponse::Ok().json(result))
}

pub async fn get_me(req: HttpRequest) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    let result = auth_core::me::get_me(dal, claims.sub).await?;
    Ok(HttpResponse::Ok().json(result))
}

pub async fn change_password(
    req: HttpRequest,
    body: web::Json<ChangePasswordRequest>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    let result =
        auth_core::me::change_password(dal, claims.sub, &body.current_password, &body.new_password)
            .await?;

    Ok(HttpResponse::Ok()
        .cookie(expired_refresh_cookie())
        .json(result))
}

pub async fn delete_account(
    req: HttpRequest,
    body: web::Json<DeleteAccountRequest>,
) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    let result = auth_core::me::delete_account(dal, claims.sub, &body.current_password).await?;

    Ok(HttpResponse::Ok()
        .cookie(expired_refresh_cookie())
        .json(result))
}

pub async fn export_account_data(req: HttpRequest) -> Result<HttpResponse, NanoServiceError> {
    let dal = get_dal(&req)?;
    let claims = crate::middleware::extract_verified_jwt(&req).await?;
    auth_core::me::register_export_dal(claims.sub, dal.clone());
    let result = auth_core::me::request_export(dal, claims.sub).await?;
    Ok(HttpResponse::Accepted().json(result))
}
