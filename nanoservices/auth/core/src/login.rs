use chrono::Utc;
use dal::{
    CreateRefreshToken, DeleteRefreshToken, GetRefreshTokenByHash,
    GetUserByEmail, GetUserById,
};
use kernel::{LoginResponse, NewRefreshToken, RefreshResponse};
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

use crate::{jwt, password, token};

fn refresh_token_expiry_days() -> i64 {
    std::env::var("REFRESH_TOKEN_EXPIRY_DAYS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30)
}

pub async fn login_user<D>(
    dal: &D,
    email: &str,
    plain_password: &str,
) -> Result<(LoginResponse, String), NanoServiceError>
where
    D: GetUserByEmail + CreateRefreshToken,
{
    let invalid_err = || {
        NanoServiceError::new(
            "Invalid email or password",
            NanoServiceErrorStatus::Unauthorized,
        )
    };

    let user = dal
        .get_user_by_email(email.to_string())
        .await?
        .ok_or_else(invalid_err)?;

    if user.email_verified_at.is_none() {
        return Err(NanoServiceError::new(
            "Email not verified. Please verify your email before logging in.",
            NanoServiceErrorStatus::Forbidden,
        ));
    }

    if !password::verify_password(plain_password, &user.password_hash)? {
        return Err(invalid_err());
    }

    let access_token = jwt::create_jwt(user.id, &user.email, true)?;
    let raw_refresh = token::generate_verification_token()?;
    let token_hash = token::hash_token(&raw_refresh);
    let expires_at = Utc::now() + chrono::Duration::days(refresh_token_expiry_days());

    dal.create_refresh_token(NewRefreshToken {
        user_id: user.id,
        token_hash,
        expires_at,
    })
    .await?;

    Ok((
        LoginResponse {
            access_token,
            token_type: "Bearer".to_string(),
        },
        raw_refresh,
    ))
}

pub async fn refresh_access_token<D>(
    dal: &D,
    raw_refresh_token: &str,
) -> Result<(RefreshResponse, String), NanoServiceError>
where
    D: GetRefreshTokenByHash + GetUserById + DeleteRefreshToken + CreateRefreshToken,
{
    let token_hash = token::hash_token(raw_refresh_token);

    let stored = dal
        .get_refresh_token_by_hash(token_hash.clone())
        .await?
        .ok_or_else(|| {
            NanoServiceError::new(
                "Invalid or revoked refresh token",
                NanoServiceErrorStatus::Unauthorized,
            )
        })?;

    if stored.expires_at < Utc::now() {
        dal.delete_refresh_token(token_hash).await?;
        return Err(NanoServiceError::new(
            "Refresh token expired",
            NanoServiceErrorStatus::Unauthorized,
        ));
    }

    let user = dal.get_user_by_id(stored.user_id).await?.ok_or_else(|| {
        NanoServiceError::new("User not found", NanoServiceErrorStatus::Unauthorized)
    })?;

    dal.delete_refresh_token(stored.token_hash.clone()).await?;

    let access_token = jwt::create_jwt(user.id, &user.email, user.email_verified_at.is_some())?;
    let new_raw = token::generate_verification_token()?;
    let new_hash = token::hash_token(&new_raw);
    let expires_at = Utc::now() + chrono::Duration::days(refresh_token_expiry_days());

    dal.create_refresh_token(NewRefreshToken {
        user_id: user.id,
        token_hash: new_hash,
        expires_at,
    })
    .await?;

    Ok((
        RefreshResponse {
            access_token,
            token_type: "Bearer".to_string(),
        },
        new_raw,
    ))
}

