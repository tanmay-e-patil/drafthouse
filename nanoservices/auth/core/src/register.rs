use dal::{CreateEmailVerificationToken, CreateUser, GetUserByEmail};
use kernel::{NewUser, RegisterResponse};
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

use crate::{email, password, token};

pub async fn register_user<D>(
    dal: &D,
    email: &str,
    password: &str,
) -> Result<RegisterResponse, NanoServiceError>
where
    D: CreateUser + GetUserByEmail + CreateEmailVerificationToken,
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
    if existing.is_some() {
        return Err(NanoServiceError::new(
            "A user with this email already exists",
            NanoServiceErrorStatus::Conflict,
        ));
    }

    let password_hash = password::hash_password(password)?;
    let new_user = NewUser {
        email: email.to_string(),
        password_hash,
    };

    let user = dal.create_user(new_user).await?;

    let verification_token = token::generate_verification_token()?;
    let token_hash = token::hash_token(&verification_token);
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(24);

    let new_token = kernel::NewEmailVerificationToken {
        user_id: user.id,
        token_hash,
        expires_at,
    };

    dal.create_email_verification_token(new_token).await?;

    if let Err(e) = email::send_verification_email(email, &verification_token).await {
        tracing::warn!("Failed to send verification email: {}", e);
    }

    Ok(RegisterResponse {
        user_id: user.id,
        email: user.email,
        message: "Registration successful. Please check your email to verify your account."
            .to_string(),
    })
}
