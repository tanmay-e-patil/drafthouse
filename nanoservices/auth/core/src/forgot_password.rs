use dal::{CreatePasswordResetToken, GetUserByEmail};
use kernel::{ForgotPasswordResponse, NewPasswordResetToken};
use utils::errors::NanoServiceError;

use crate::{email, token};

pub async fn forgot_password<D>(dal: &D, email: &str) -> Result<ForgotPasswordResponse, NanoServiceError>
where
    D: GetUserByEmail + CreatePasswordResetToken,
{
    let user = match dal.get_user_by_email(email.to_string()).await? {
        Some(u) => u,
        None => {
            // Still return success to prevent email enumeration
            return Ok(ForgotPasswordResponse {
                message: "If an account exists, a password reset link has been sent.".to_string(),
            });
        }
    };

    let raw_token = token::generate_verification_token()?;
    let token_hash = token::hash_token(&raw_token);
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(1);

    dal.create_password_reset_token(NewPasswordResetToken {
        user_id: user.id,
        token_hash,
        expires_at,
    })
    .await?;

    email::send_password_reset_email(&user.email, &raw_token).await?;

    Ok(ForgotPasswordResponse {
        message: "If an account exists, a password reset link has been sent.".to_string(),
    })
}
