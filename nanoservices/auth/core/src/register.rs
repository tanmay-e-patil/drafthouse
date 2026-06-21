use dal::{
    CreateEmailVerificationToken, CreateUser, GetUserByEmail, InvalidateEmailVerificationTokens,
};
use kernel::{NewUser, RegisterResponse};
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

use crate::{email, password, token};

pub async fn register_user<D>(
    dal: &D,
    email: &str,
    password: &str,
) -> Result<RegisterResponse, NanoServiceError>
where
    D: CreateUser
        + GetUserByEmail
        + CreateEmailVerificationToken
        + InvalidateEmailVerificationTokens,
{
    if email.trim().is_empty() {
        return Err(NanoServiceError::new(
            "Email is required",
            NanoServiceErrorStatus::BadRequest,
        ));
    }
    if password.len() < 8 {
        return Err(NanoServiceError::new(
            "Password must be at least 8 characters",
            NanoServiceErrorStatus::BadRequest,
        ));
    }

    let existing = dal.get_user_by_email(email.to_string()).await?;
    if let Some(user) = existing {
        if user.email_verified_at.is_some() {
            return Err(NanoServiceError::new(
                "An account with this email already exists. Please sign in.",
                NanoServiceErrorStatus::Conflict,
            ));
        }

        dal.invalidate_email_verification_tokens(user.id).await?;
        send_verification(dal, user.id, email).await?;
        return Ok(RegisterResponse {
            user_id: user.id,
            email: user.email,
            message: "We sent a fresh verification email. Please check your inbox.".to_string(),
        });
    }

    let password_hash = password::hash_password(password)?;
    let new_user = NewUser {
        email: email.to_string(),
        password_hash,
    };

    let user = dal.create_user(new_user).await?;
    send_verification(dal, user.id, email).await?;

    Ok(RegisterResponse {
        user_id: user.id,
        email: user.email,
        message: "Registration successful. Please check your email to verify your account."
            .to_string(),
    })
}

async fn send_verification<D>(
    dal: &D,
    user_id: uuid::Uuid,
    email: &str,
) -> Result<(), NanoServiceError>
where
    D: CreateEmailVerificationToken,
{
    let verification_token = token::generate_verification_token()?;
    let token_hash = token::hash_token(&verification_token);
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(24);

    dal.create_email_verification_token(kernel::NewEmailVerificationToken {
        user_id,
        token_hash,
        expires_at,
    })
    .await?;

    if let Err(e) = email::send_verification_email(email, &verification_token).await {
        tracing::warn!("Failed to send verification email: {}", e);
        return Err(NanoServiceError::new(
            "Account created, but we couldn't send the verification email. Please request a new verification email.",
            NanoServiceErrorStatus::InternalServerError,
        ));
    }

    Ok(())
}
