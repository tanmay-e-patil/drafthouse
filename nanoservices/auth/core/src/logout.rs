use dal::DeleteRefreshToken;
use utils::errors::NanoServiceError;

use crate::token;

pub async fn logout<D>(dal: &D, raw_refresh_token: &str) -> Result<(), NanoServiceError>
where
    D: DeleteRefreshToken,
{
    let token_hash = token::hash_token(raw_refresh_token);
    dal.delete_refresh_token(token_hash).await
}
