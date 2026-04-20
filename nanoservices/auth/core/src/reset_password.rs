use dal::{
    DeleteAllRefreshTokensForUser, GetPasswordResetToken, MarkPasswordResetTokenUsed,
    UpdateUserPassword,
};
use kernel::ForgotPasswordResponse;
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

use crate::{password, token};

pub async fn reset_password<D>(
    dal: &D,
    raw_token: &str,
    new_password_plain: &str,
) -> Result<ForgotPasswordResponse, NanoServiceError>
where
    D: GetPasswordResetToken
        + UpdateUserPassword
        + MarkPasswordResetTokenUsed
        + DeleteAllRefreshTokensForUser,
{
    let token_hash = token::hash_token(raw_token);

    let stored_token = dal
        .get_password_reset_token(token_hash.clone())
        .await?
        .ok_or_else(|| {
            NanoServiceError::new(
                "Invalid or expired password reset token",
                NanoServiceErrorStatus::BadRequest,
            )
        })?;

    if stored_token.used_at.is_some() {
        return Err(NanoServiceError::new(
            "This password reset link has already been used",
            NanoServiceErrorStatus::BadRequest,
        ));
    }

    if stored_token.expires_at < chrono::Utc::now() {
        return Err(NanoServiceError::new(
            "This password reset link has expired",
            NanoServiceErrorStatus::BadRequest,
        ));
    }

    let hashed_password = password::hash_password(new_password_plain)?;

    dal.update_user_password(stored_token.user_id, hashed_password)
        .await?;

    dal.mark_password_reset_token_used(token_hash).await?;

    // Invalidate all existing sessions
    let _ = dal.delete_all_refresh_tokens_for_user(stored_token.user_id).await;

    Ok(ForgotPasswordResponse {
        message: "Your password has been successfully reset. You can now log in with your new password.".to_string(),
    })
}
