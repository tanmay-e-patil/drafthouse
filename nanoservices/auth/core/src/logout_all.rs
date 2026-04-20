use dal::DeleteAllRefreshTokensForUser;
use utils::errors::NanoServiceError;

pub async fn logout_all<D>(dal: &D, user_id: uuid::Uuid) -> Result<(), NanoServiceError>
where
    D: DeleteAllRefreshTokensForUser,
{
    dal.delete_all_refresh_tokens_for_user(user_id).await
}
