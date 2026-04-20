use chrono::Utc;
use dal::{GetEmailVerificationToken, MarkUserVerified};
use kernel::VerifyEmailResponse;
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

use crate::token;

pub async fn verify_email<D>(
    dal: &D,
    raw_token: &str,
) -> Result<VerifyEmailResponse, NanoServiceError>
where
    D: GetEmailVerificationToken + MarkUserVerified,
{
    if raw_token.trim().is_empty() {
        return Err(NanoServiceError::new(
            "Token is required",
            NanoServiceErrorStatus::BadRequest,
        ));
    }

    let token_hash = token::hash_token(raw_token);
    let stored = dal.get_email_verification_token(token_hash).await?;

    match stored {
        None => Err(NanoServiceError::new(
            "Invalid or expired verification token",
            NanoServiceErrorStatus::BadRequest,
        )),
        Some(token) => {
            if token.expires_at < Utc::now() {
                return Err(NanoServiceError::new(
                    "Verification token has expired",
                    NanoServiceErrorStatus::BadRequest,
                ));
            }

            dal.mark_user_verified(token.user_id).await?;

            Ok(VerifyEmailResponse {
                message: "Email verified successfully. You can now log in.".to_string(),
            })
        }
    }
}
