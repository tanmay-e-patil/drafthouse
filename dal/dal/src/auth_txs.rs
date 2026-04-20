use kernel::{
    EmailVerificationToken, NewEmailVerificationToken, NewPasswordResetToken, NewRefreshToken,
    NewUser, PasswordResetToken, RefreshToken, User,
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
    DeleteAllRefreshTokensForUser => delete_all_refresh_tokens_for_user(user_id: uuid::Uuid) -> (),
    CreatePasswordResetToken => create_password_reset_token(new_token: NewPasswordResetToken) -> PasswordResetToken,
    GetPasswordResetToken => get_password_reset_token(token_hash: String) -> Option<PasswordResetToken>,
    MarkPasswordResetTokenUsed => mark_password_reset_token_used(token_hash: String) -> (),
    UpdateUserPassword => update_user_password(user_id: uuid::Uuid, password_hash: String) -> ()
);
