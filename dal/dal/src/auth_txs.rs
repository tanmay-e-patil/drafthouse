use kernel::{EmailVerificationToken, NewEmailVerificationToken, NewUser, User};

crate::define_dal_transactions!(
    CreateUser => create_user(new_user: NewUser) -> User,
    GetUserByEmail => get_user_by_email(email: String) -> Option<User>,
    GetUserById => get_user_by_id(id: uuid::Uuid) -> Option<User>,
    CreateEmailVerificationToken => create_email_verification_token(new_token: NewEmailVerificationToken) -> EmailVerificationToken,
    GetEmailVerificationToken => get_email_verification_token(token_hash: String) -> Option<EmailVerificationToken>,
    InvalidateEmailVerificationTokens => invalidate_email_verification_tokens(user_id: uuid::Uuid) -> (),
    MarkUserVerified => mark_user_verified(user_id: uuid::Uuid) -> ()
);
