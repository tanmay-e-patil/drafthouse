use actix_web::HttpRequest;
use kernel::JwtClaims;
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

pub async fn extract_verified_jwt(req: &HttpRequest) -> Result<JwtClaims, NanoServiceError> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            NanoServiceError::new(
                "Missing authorization header",
                NanoServiceErrorStatus::Unauthorized,
            )
        })?;

    if !auth_header.starts_with("Bearer ") {
        return Err(NanoServiceError::new(
            "Invalid authorization header format",
            NanoServiceErrorStatus::Unauthorized,
        ));
    }

    let token = auth_header.strip_prefix("Bearer ").unwrap();
    let claims = auth_core::jwt::verify_jwt(token)?;
    auth_core::jwt::require_verified(&claims)?;
    Ok(claims)
}

pub async fn extract_optional_verified_jwt(
    req: &HttpRequest,
) -> Result<Option<JwtClaims>, NanoServiceError> {
    match req.headers().get("Authorization") {
        Some(_) => extract_verified_jwt(req).await.map(Some),
        None => Ok(None),
    }
}
