use kernel::{
    EmailVerificationToken, NewEmailVerificationToken, NewRefreshToken, NewUser, RefreshToken, User,
};

crate::define_dal_transactions!(
    CreateUser => create_user(new_user: NewUser) -> User,
    GetUserByEmail => get_user_by_email(email: String) -> Option<User>,
    GetUserById => get_user_by_id(id: uuid::Uuid) -> Option<User>,
    CreateEmailVerificationToken => create_email_verification_token(new_token: NewEmailVerificationToken) -> EmailVerificationToken,
    GetEmailVerificationToken => get_email_verification_token(token_hash: String) -> Option<EmailVerificationToken>,
    InvalidateEmailVerificationTokens => invalidate_email_verification_tokens(user_id: uuid::Uuid) -> (),
    MarkUserVerified => mark_user_verified(user_id: uuid::Uuid) -> (),
    CreateRefreshToken => create_refresh_token(new_token: NewRefreshToken) -> RefreshToken,
    GetRefreshTokenByHash => get_refresh_token_by_hash(token_hash: String) -> Option<RefreshToken>,
    DeleteRefreshToken => delete_refresh_token(token_hash: String) -> (),
    DeleteAllRefreshTokensForUser => delete_all_refresh_tokens_for_user(user_id: uuid::Uuid) -> ()
);
