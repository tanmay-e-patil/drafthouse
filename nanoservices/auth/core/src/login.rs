use chrono::Utc;
use dal::{
    CreateRefreshToken, DeleteAllRefreshTokensForUser, DeleteRefreshToken, GetRefreshTokenByHash,
    GetUserByEmail, GetUserById, MarkWelcomeDocCreated,
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
) -> Result<(LoginResponse, String, uuid::Uuid, bool), NanoServiceError>
where
    D: GetUserByEmail + CreateRefreshToken + MarkWelcomeDocCreated,
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

    let is_first_login = !user.welcome_doc_created;
    if is_first_login {
        dal.mark_welcome_doc_created(user.id).await?;
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
            welcome_doc_id: None,
        },
        raw_refresh,
        user.id,
        is_first_login,
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

pub async fn logout<D>(dal: &D, raw_refresh_token: &str) -> Result<(), NanoServiceError>
where
    D: DeleteRefreshToken,
{
    let token_hash = token::hash_token(raw_refresh_token);
    dal.delete_refresh_token(token_hash).await
}

pub async fn logout_all<D>(dal: &D, user_id: uuid::Uuid) -> Result<(), NanoServiceError>
where
    D: DeleteAllRefreshTokensForUser,
{
    dal.delete_all_refresh_tokens_for_user(user_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use kernel::{NewRefreshToken, RefreshToken, User};
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    fn verified_user() -> User {
        User {
            id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
            password_hash: password::hash_password("password123").unwrap(),
            email_verified_at: Some(Utc::now()),
            created_at: Utc::now(),
            welcome_doc_created: false,
        }
    }

    fn verified_user_returning() -> User {
        User {
            id: Uuid::new_v4(),
            email: "returning@example.com".to_string(),
            password_hash: password::hash_password("password123").unwrap(),
            email_verified_at: Some(Utc::now()),
            created_at: Utc::now(),
            welcome_doc_created: true,
        }
    }

    fn unverified_user() -> User {
        User {
            id: Uuid::new_v4(),
            email: "unverified@example.com".to_string(),
            password_hash: password::hash_password("password123").unwrap(),
            email_verified_at: None,
            created_at: Utc::now(),
            welcome_doc_created: false,
        }
    }

    struct MockDal {
        user: Option<User>,
        refresh_tokens: Arc<Mutex<Vec<RefreshToken>>>,
        welcome_marked: Arc<Mutex<Vec<Uuid>>>,
    }

    impl MockDal {
        fn with_user(user: User) -> Self {
            Self {
                user: Some(user),
                refresh_tokens: Arc::new(Mutex::new(vec![])),
                welcome_marked: Arc::new(Mutex::new(vec![])),
            }
        }

        fn with_user_and_token(user: User, token: RefreshToken) -> Self {
            Self {
                user: Some(user),
                refresh_tokens: Arc::new(Mutex::new(vec![token])),
                welcome_marked: Arc::new(Mutex::new(vec![])),
            }
        }

        fn empty() -> Self {
            Self {
                user: None,
                refresh_tokens: Arc::new(Mutex::new(vec![])),
                welcome_marked: Arc::new(Mutex::new(vec![])),
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

    impl GetUserById for MockDal {
        fn get_user_by_id(
            &self,
            _id: Uuid,
        ) -> impl std::future::Future<Output = Result<Option<User>, NanoServiceError>> + Send
        {
            let user = self.user.clone();
            async move { Ok(user) }
        }
    }

    impl CreateRefreshToken for MockDal {
        fn create_refresh_token(
            &self,
            new_token: NewRefreshToken,
        ) -> impl std::future::Future<Output = Result<RefreshToken, NanoServiceError>> + Send
        {
            let tokens = Arc::clone(&self.refresh_tokens);
            async move {
                let rt = RefreshToken {
                    id: Uuid::new_v4(),
                    user_id: new_token.user_id,
                    token_hash: new_token.token_hash,
                    expires_at: new_token.expires_at,
                };
                tokens.lock().unwrap().push(rt.clone());
                Ok(rt)
            }
        }
    }

    impl GetRefreshTokenByHash for MockDal {
        fn get_refresh_token_by_hash(
            &self,
            token_hash: String,
        ) -> impl std::future::Future<Output = Result<Option<RefreshToken>, NanoServiceError>> + Send
        {
            let tokens = Arc::clone(&self.refresh_tokens);
            async move {
                let found = tokens
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|t| t.token_hash == token_hash)
                    .cloned();
                Ok(found)
            }
        }
    }

    impl DeleteRefreshToken for MockDal {
        fn delete_refresh_token(
            &self,
            token_hash: String,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let tokens = Arc::clone(&self.refresh_tokens);
            async move {
                tokens
                    .lock()
                    .unwrap()
                    .retain(|t| t.token_hash != token_hash);
                Ok(())
            }
        }
    }

    impl DeleteAllRefreshTokensForUser for MockDal {
        fn delete_all_refresh_tokens_for_user(
            &self,
            user_id: Uuid,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let tokens = Arc::clone(&self.refresh_tokens);
            async move {
                tokens.lock().unwrap().retain(|t| t.user_id != user_id);
                Ok(())
            }
        }
    }

    impl dal::MarkWelcomeDocCreated for MockDal {
        fn mark_welcome_doc_created(
            &self,
            user_id: Uuid,
        ) -> impl std::future::Future<Output = Result<(), NanoServiceError>> + Send {
            let marked = Arc::clone(&self.welcome_marked);
            async move {
                marked.lock().unwrap().push(user_id);
                Ok(())
            }
        }
    }

    // ── login_user ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn login_valid_verified_returns_token_pair() {
        let user = verified_user();
        let dal = MockDal::with_user(user);
        let result = login_user(&dal, "test@example.com", "password123").await;
        assert!(result.is_ok());
        let (resp, raw_refresh, _, _) = result.unwrap();
        assert_eq!(resp.token_type, "Bearer");
        assert!(!resp.access_token.is_empty());
        assert!(!raw_refresh.is_empty());
    }

    #[tokio::test]
    async fn login_wrong_password_returns_401() {
        let user = verified_user();
        let dal = MockDal::with_user(user);
        let result = login_user(&dal, "test@example.com", "wrongpassword").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.status, NanoServiceErrorStatus::Unauthorized);
    }

    #[tokio::test]
    async fn login_unknown_email_returns_401() {
        let dal = MockDal::empty();
        let result = login_user(&dal, "ghost@example.com", "password123").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.status, NanoServiceErrorStatus::Unauthorized);
    }

    #[tokio::test]
    async fn login_unverified_returns_403() {
        let user = unverified_user();
        let dal = MockDal::with_user(user);
        let result = login_user(&dal, "unverified@example.com", "password123").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.status, NanoServiceErrorStatus::Forbidden);
    }

    #[tokio::test]
    async fn login_stores_refresh_token_in_dal() {
        let user = verified_user();
        let dal = MockDal::with_user(user);
        let _ = login_user(&dal, "test@example.com", "password123")
            .await
            .unwrap();
        assert_eq!(dal.refresh_tokens.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn first_login_returns_is_first_login_true_and_marks_flag() {
        let user = verified_user();
        let user_id = user.id;
        let dal = MockDal::with_user(user);
        let (_, _, returned_user_id, is_first) =
            login_user(&dal, "test@example.com", "password123")
                .await
                .unwrap();
        assert!(is_first);
        assert_eq!(returned_user_id, user_id);
        assert_eq!(dal.welcome_marked.lock().unwrap().len(), 1);
        assert_eq!(dal.welcome_marked.lock().unwrap()[0], user_id);
    }

    #[tokio::test]
    async fn returning_login_returns_is_first_login_false_and_does_not_remark() {
        let user = verified_user_returning();
        let dal = MockDal::with_user(user);
        let (_, _, _, is_first) = login_user(&dal, "returning@example.com", "password123")
            .await
            .unwrap();
        assert!(!is_first);
        assert!(dal.welcome_marked.lock().unwrap().is_empty());
    }

    // ── refresh_access_token ───────────────────────────────────────────────────

    #[tokio::test]
    async fn refresh_valid_token_returns_new_pair() {
        let user = verified_user();
        let raw = token::generate_verification_token().unwrap();
        let hash = token::hash_token(&raw);
        let stored_token = RefreshToken {
            id: Uuid::new_v4(),
            user_id: user.id,
            token_hash: hash,
            expires_at: Utc::now() + Duration::days(30),
        };
        let dal = MockDal::with_user_and_token(user, stored_token);

        let result = refresh_access_token(&dal, &raw).await;
        assert!(result.is_ok());
        let (resp, new_raw) = result.unwrap();
        assert_eq!(resp.token_type, "Bearer");
        assert!(!new_raw.is_empty());
    }

    #[tokio::test]
    async fn refresh_rotates_token_old_one_deleted() {
        let user = verified_user();
        let raw = token::generate_verification_token().unwrap();
        let hash = token::hash_token(&raw);
        let stored_token = RefreshToken {
            id: Uuid::new_v4(),
            user_id: user.id,
            token_hash: hash.clone(),
            expires_at: Utc::now() + Duration::days(30),
        };
        let dal = MockDal::with_user_and_token(user, stored_token);

        let _ = refresh_access_token(&dal, &raw).await.unwrap();

        let tokens = dal.refresh_tokens.lock().unwrap();
        assert_eq!(tokens.len(), 1);
        assert_ne!(tokens[0].token_hash, hash, "old token should be gone");
    }

    #[tokio::test]
    async fn refresh_invalid_hash_returns_401() {
        let user = verified_user();
        let dal = MockDal::with_user(user);
        let result = refresh_access_token(&dal, "nonexistent_token").await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::Unauthorized
        );
    }

    #[tokio::test]
    async fn refresh_expired_token_returns_401() {
        let user = verified_user();
        let raw = token::generate_verification_token().unwrap();
        let hash = token::hash_token(&raw);
        let stored_token = RefreshToken {
            id: Uuid::new_v4(),
            user_id: user.id,
            token_hash: hash,
            expires_at: Utc::now() - Duration::hours(1),
        };
        let dal = MockDal::with_user_and_token(user, stored_token);

        let result = refresh_access_token(&dal, &raw).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            NanoServiceErrorStatus::Unauthorized
        );
    }
}
