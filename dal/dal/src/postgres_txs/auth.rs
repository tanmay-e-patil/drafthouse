use crate::auth_txs::{
    CreateEmailVerificationToken, CreateUser, GetEmailVerificationToken, GetUserByEmail,
    GetUserById, InvalidateEmailVerificationTokens, MarkUserVerified,
};
use dal_tx_impl::impl_transaction;
use kernel::{EmailVerificationToken, NewEmailVerificationToken, NewUser, User};
use sqlx::PgPool;
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

pub struct SqlxPostGresDescriptor {
    pub pool: PgPool,
}

#[impl_transaction(SqlxPostGresDescriptor, CreateUser, create_user)]
async fn create_user(&self, new_user: NewUser) -> Result<User, NanoServiceError> {
    let row = sqlx::query_as::<_, User>(
        "INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id, email, password_hash, email_verified_at, created_at",
    )
    .bind(&new_user.email)
    .bind(&new_user.password_hash)
    .fetch_one(&self.pool)
    .await
    .map_err(|e| NanoServiceError::new(format!("Failed to create user: {}", e), NanoServiceErrorStatus::InternalServerError))?;

    Ok(row)
}

#[impl_transaction(SqlxPostGresDescriptor, GetUserByEmail, get_user_by_email)]
async fn get_user_by_email(&self, email: String) -> Result<Option<User>, NanoServiceError> {
    let row = sqlx::query_as::<_, User>(
        "SELECT id, email, password_hash, email_verified_at, created_at FROM users WHERE email = $1",
    )
    .bind(&email)
    .fetch_optional(&self.pool)
    .await
    .map_err(|e| NanoServiceError::new(format!("Failed to get user: {}", e), NanoServiceErrorStatus::InternalServerError))?;

    Ok(row)
}

#[impl_transaction(SqlxPostGresDescriptor, GetUserById, get_user_by_id)]
async fn get_user_by_id(&self, id: uuid::Uuid) -> Result<Option<User>, NanoServiceError> {
    let row = sqlx::query_as::<_, User>(
        "SELECT id, email, password_hash, email_verified_at, created_at FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&self.pool)
    .await
    .map_err(|e| {
        NanoServiceError::new(
            format!("Failed to get user by id: {}", e),
            NanoServiceErrorStatus::InternalServerError,
        )
    })?;

    Ok(row)
}

#[impl_transaction(
    SqlxPostGresDescriptor,
    CreateEmailVerificationToken,
    create_email_verification_token
)]
async fn create_email_verification_token(
    &self,
    new_token: NewEmailVerificationToken,
) -> Result<EmailVerificationToken, NanoServiceError> {
    let row = sqlx::query_as::<_, EmailVerificationToken>(
        "INSERT INTO email_verification_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3) RETURNING id, user_id, token_hash, expires_at",
    )
    .bind(new_token.user_id)
    .bind(&new_token.token_hash)
    .bind(new_token.expires_at)
    .fetch_one(&self.pool)
    .await
    .map_err(|e| NanoServiceError::new(format!("Failed to create verification token: {}", e), NanoServiceErrorStatus::InternalServerError))?;

    Ok(row)
}

#[impl_transaction(
    SqlxPostGresDescriptor,
    GetEmailVerificationToken,
    get_email_verification_token
)]
async fn get_email_verification_token(
    &self,
    token_hash: String,
) -> Result<Option<EmailVerificationToken>, NanoServiceError> {
    let row = sqlx::query_as::<_, EmailVerificationToken>(
        "SELECT id, user_id, token_hash, expires_at FROM email_verification_tokens WHERE token_hash = $1",
    )
    .bind(&token_hash)
    .fetch_optional(&self.pool)
    .await
    .map_err(|e| NanoServiceError::new(format!("Failed to get verification token: {}", e), NanoServiceErrorStatus::InternalServerError))?;

    Ok(row)
}

#[impl_transaction(
    SqlxPostGresDescriptor,
    InvalidateEmailVerificationTokens,
    invalidate_email_verification_tokens
)]
async fn invalidate_email_verification_tokens(
    &self,
    user_id: uuid::Uuid,
) -> Result<(), NanoServiceError> {
    utils::safe_eject!(
        sqlx::query("DELETE FROM email_verification_tokens WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await,
        NanoServiceErrorStatus::InternalServerError,
        "Failed to invalidate verification tokens"
    )?;
    Ok(())
}

#[impl_transaction(SqlxPostGresDescriptor, MarkUserVerified, mark_user_verified)]
async fn mark_user_verified(&self, user_id: uuid::Uuid) -> Result<(), NanoServiceError> {
    utils::safe_eject!(
        sqlx::query("UPDATE users SET email_verified_at = now() WHERE id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await,
        NanoServiceErrorStatus::InternalServerError,
        "Failed to mark user verified"
    )?;
    Ok(())
}
