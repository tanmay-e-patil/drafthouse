use dal::{CreateEmailVerificationToken, GetUserByEmail, InvalidateEmailVerificationTokens};
use kernel::ResendVerificationResponse;
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

use crate::{email, token};

pub async fn resend_verification<D>(
    dal: &D,
    email: &str,
) -> Result<ResendVerificationResponse, NanoServiceError>
where
    D: GetUserByEmail + CreateEmailVerificationToken + InvalidateEmailVerificationTokens,
{
    if email.trim().is_empty() {
        return Err(NanoServiceError::new(
            "Email is required",
            NanoServiceErrorStatus::BadRequest,
        ));
    }

    let user = dal
        .get_user_by_email(email.to_string())
        .await?
        .ok_or_else(|| {
            NanoServiceError::new(
                "No account found with this email",
                NanoServiceErrorStatus::BadRequest,
            )
        })?;

    if user.email_verified_at.is_some() {
        return Err(NanoServiceError::new(
            "Email is already verified",
            NanoServiceErrorStatus::BadRequest,
        ));
    }

    dal.invalidate_email_verification_tokens(user.id).await?;

    let verification_token = token::generate_verification_token()?;
    let token_hash = token::hash_token(&verification_token);
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(24);

    let new_token = kernel::NewEmailVerificationToken {
        user_id: user.id,
        token_hash,
        expires_at,
    };

    dal.create_email_verification_token(new_token).await?;

    email::send_verification_email(email, &verification_token).await?;

    Ok(ResendVerificationResponse {
        message: "A new verification email has been sent. Please check your inbox.".to_string(),
    })
}
