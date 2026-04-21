use chrono::Utc;
use dal::{
    CreatePasswordResetToken, DeleteAllRefreshTokensForUser, GetPasswordResetToken, GetUserByEmail,
    MarkPasswordResetTokenUsed, UpdateUserPassword,
};
use kernel::{ForgotPasswordResponse, ResetPasswordResponse};
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

use crate::{email, password, token};

const RESET_TOKEN_EXPIRY_MINUTES: i64 = 15;

pub async fn forgot_password<D>(
    dal: &D,
    email_addr: &str,
) -> Result<ForgotPasswordResponse, NanoServiceError>
where
    D: GetUserByEmail + CreatePasswordResetToken,
{
    let user = match dal.get_user_by_email(email_addr.to_string()).await? {
        Some(u) => u,
        None => {
            return Ok(ForgotPasswordResponse {
                message: "If an account with that email exists, a reset link has been sent."
                    .to_string(),
            });
        }
    };

    let raw_token = token::generate_verification_token()?;
    let token_hash = token::hash_token(&raw_token);
    let expires_at = Utc::now() + chrono::Duration::minutes(RESET_TOKEN_EXPIRY_MINUTES);

    dal.create_password_reset_token(kernel::NewPasswordResetToken {
        user_id: user.id,
        token_hash,
        expires_at,
    })
    .await?;

    if let Err(e) = email::send_password_reset_email(email_addr, &raw_token).await {
        tracing::warn!("Failed to send password reset email: {}", e);
    }

    Ok(ForgotPasswordResponse {
        message: "If an account with that email exists, a reset link has been sent.".to_string(),
    })
}

pub async fn reset_password<D>(
    dal: &D,
    raw_token: &str,
    new_password: &str,
) -> Result<ResetPasswordResponse, NanoServiceError>
where
    D: GetPasswordResetToken
        + MarkPasswordResetTokenUsed
        + UpdateUserPassword
        + DeleteAllRefreshTokensForUser,
{
    let token_hash = token::hash_token(raw_token);

    let stored = dal
        .get_password_reset_token(token_hash)
        .await?
        .ok_or_else(|| {
            NanoServiceError::new(
                "Invalid or expired reset token",
                NanoServiceErrorStatus::BadRequest,
            )
        })?;

    if stored.expires_at < Utc::now() {
        return Err(NanoServiceError::new(
            "Reset token has expired",
            NanoServiceErrorStatus::BadRequest,
        ));
    }

    if stored.used_at.is_some() {
        return Err(NanoServiceError::new(
            "Reset token has already been used",
            NanoServiceErrorStatus::BadRequest,
        ));
    }

    let new_hash = password::hash_password(new_password)?;

    dal.update_user_password(stored.user_id, new_hash).await?;
    dal.mark_password_reset_token_used(token::hash_token(raw_token))
        .await?;
    dal.delete_all_refresh_tokens_for_user(stored.user_id)
        .await?;

    Ok(ResetPasswordResponse {
        message: "Password has been reset successfully.".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use kernel::{NewPasswordResetToken, PasswordResetToken, User};
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    fn test_user() -> User {
        User {
            id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
            password_hash: password::hash_password("oldpassword").unwrap(),
            email_verified_at: Some(Utc::now()),
            created_at: Utc::now(),
            welcome_doc_created: false,
        }
    }

    struct MockDal {
        user: Option<User>,
        password_reset_tokens: Arc<Mutex<Vec<PasswordResetToken>>>,
        password_updated: Arc<Mutex<Option<(Uuid, String)>>>,
        sessions_revoked_for: Arc<Mutex<Vec<Uuid>>>,
    }

    impl MockDal {
        fn with_user(user: User) -> Self {
            Self {
                user: Some(user),
                password_reset_tokens: Arc::new(Mutex::new(vec![])),
                password_updated: Arc::new(Mutex::new(None)),
                sessions_revoked_for: Arc::new(Mutex::new(vec![])),
            }
        }

        fn with_user_and_token(user: User, token: PasswordResetToken) -> Self {
            Self {
                user: Some(user),
                password_reset_tokens: Arc::new(Mutex::new(vec![token])),
                password_updated: Arc::new(Mutex::new(None)),
                sessions_revoked_for: Arc::new(Mutex::new(vec![])),
            }
        }

        fn empty() -> Self {
            Self {
                user: None,
                password_reset_tokens: Arc::new(Mutex::new(vec![])),
                password_updated: Arc::new(Mutex::new(None)),
                sessions_revoked_for: Arc::new(Mutex::new(vec![])),
            }
        }
    }

    impl GetUserByEmail for MockDal {
        fn get_user_by_email(
            &self,
            _email: String,
        ) -> impl std::future::Future<Output = Result<Option<User>, NanoServiceError>> + Send
        {
            let user = self.user.clone();
            async move { Ok(user) }
        }
    }

    impl CreatePasswordResetToken for MockDal {
        fn create_password_reset_token(
            &self,
            new_token: NewPasswordResetToken,
        ) -> impl std::future::Future<Output = Result<PasswordResetToken, NanoServiceError>> + Send
        {
            let tokens = Arc::clone(&self.password_reset_tokens);
            async move {
                let t = PasswordResetToken {
                    id: Uuid::new_v4(),
                    user_id: new_token.user_id,
                    token_hash: new_token.token_hash,
                    expires_at: new_token.expires_at,
                    used_at: None,
                };
                tokens.lock().unwrap().push(t.clone());
                Ok(t)
            }
        }
    }

    impl GetPasswordResetToken for MockDal {
        fn get_password_reset_token(
            &self,
            token_hash: String,
        ) -> impl std::future::Future<Output = Result<Option<PasswordResetToken>, NanoServiceError>> + Send
        {
            let tokens = Arc::clone(&self.password_reset_tokens);
            async move {
                Ok(tokens
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|t| t.token_hash == token_hash)
                    .cloned())
            }
        }
    }

    impl MarkPasswordResetTokenUsed for MockDal {
        fn mark_password_reset_token_used(
            &self,
            token_hash: String,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let tokens = Arc::clone(&self.password_reset_tokens);
            async move {
                let mut tokens = tokens.lock().unwrap();
                if let Some(t) = tokens.iter_mut().find(|t| t.token_hash == token_hash) {
                    t.used_at = Some(Utc::now());
                }
                Ok(())
            }
        }
    }

    impl UpdateUserPassword for MockDal {
        fn update_user_password(
            &self,
            user_id: Uuid,
            password_hash: String,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let updated = Arc::clone(&self.password_updated);
            async move {
                *updated.lock().unwrap() = Some((user_id, password_hash));
                Ok(())
            }
        }
    }

    impl DeleteAllRefreshTokensForUser for MockDal {
        fn delete_all_refresh_tokens_for_user(
            &self,
            user_id: Uuid,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let revoked = Arc::clone(&self.sessions_revoked_for);
            async move {
                revoked.lock().unwrap().push(user_id);
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn forgot_password_unknown_email_returns_200_generic_message() {
        let dal = MockDal::empty();
        let result = forgot_password(&dal, "ghost@example.com").await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert!(resp.message.contains("reset link"));
    }

    #[tokio::test]
    async fn forgot_password_known_email_creates_token_and_returns_200() {
        let user = test_user();
        let dal = MockDal::with_user(user);
        let result = forgot_password(&dal, "test@example.com").await;
        assert!(result.is_ok());
        assert_eq!(dal.password_reset_tokens.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn reset_password_success_updates_password_and_revokes_sessions() {
        let user = test_user();
        let raw = token::generate_verification_token().unwrap();
        let hash = token::hash_token(&raw);
        let stored_token = PasswordResetToken {
            id: Uuid::new_v4(),
            user_id: user.id,
            token_hash: hash,
            expires_at: Utc::now() + Duration::minutes(15),
            used_at: None,
        };
        let dal = MockDal::with_user_and_token(user, stored_token);

        let result = reset_password(&dal, &raw, "brandNewPassword123").await;
        assert!(result.is_ok());

        assert!(dal.password_updated.lock().unwrap().is_some());
        assert_eq!(dal.sessions_revoked_for.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn reset_password_expired_token_returns_400() {
        let user = test_user();
        let raw = token::generate_verification_token().unwrap();
        let hash = token::hash_token(&raw);
        let stored_token = PasswordResetToken {
            id: Uuid::new_v4(),
            user_id: user.id,
            token_hash: hash,
            expires_at: Utc::now() - Duration::minutes(1),
            used_at: None,
        };
        let dal = MockDal::with_user_and_token(user, stored_token);

        let result = reset_password(&dal, &raw, "newPassword123").await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::BadRequest
        );
    }

    #[tokio::test]
    async fn reset_password_used_token_returns_400() {
        let user = test_user();
        let raw = token::generate_verification_token().unwrap();
        let hash = token::hash_token(&raw);
        let stored_token = PasswordResetToken {
            id: Uuid::new_v4(),
            user_id: user.id,
            token_hash: hash,
            expires_at: Utc::now() + Duration::minutes(15),
            used_at: Some(Utc::now() - Duration::minutes(5)),
        };
        let dal = MockDal::with_user_and_token(user, stored_token);

        let result = reset_password(&dal, &raw, "newPassword123").await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::BadRequest
        );
    }

    #[tokio::test]
    async fn reset_password_invalid_token_returns_400() {
        let dal = MockDal::empty();

        let result = reset_password(&dal, "nonexistent_token", "newPassword123").await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::BadRequest
        );
    }

    #[tokio::test]
    async fn reset_password_marks_token_as_used() {
        let user = test_user();
        let raw = token::generate_verification_token().unwrap();
        let hash = token::hash_token(&raw);
        let stored_token = PasswordResetToken {
            id: Uuid::new_v4(),
            user_id: user.id,
            token_hash: hash.clone(),
            expires_at: Utc::now() + Duration::minutes(15),
            used_at: None,
        };
        let dal = MockDal::with_user_and_token(user, stored_token);

        reset_password(&dal, &raw, "brandNewPassword123")
            .await
            .unwrap();

        let token_after = dal
            .password_reset_tokens
            .lock()
            .unwrap()
            .iter()
            .find(|t| t.token_hash == hash)
            .cloned();
        assert!(token_after.is_some());
        assert!(token_after.unwrap().used_at.is_some());
    }
}
