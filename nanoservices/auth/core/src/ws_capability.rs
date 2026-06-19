use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};
use uuid::Uuid;

const DEFAULT_WS_CAPABILITY_TTL_SECS: usize = 30;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WsCapabilityClaims {
    pub sub: Uuid,
    pub doc_id: Uuid,
    pub readonly: bool,
    pub exp: usize,
    pub iat: usize,
}

fn ttl_secs() -> usize {
    std::env::var("WS_CAPABILITY_TTL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_WS_CAPABILITY_TTL_SECS)
}

pub fn create_ws_capability(
    user_id: Uuid,
    doc_id: Uuid,
    readonly: bool,
) -> Result<String, NanoServiceError> {
    let iat = Utc::now().timestamp() as usize;
    let claims = WsCapabilityClaims {
        sub: user_id,
        doc_id,
        readonly,
        exp: iat + ttl_secs(),
        iat,
    };

    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(super::jwt::jwt_secret().as_bytes()),
    )
    .map_err(|e| {
        NanoServiceError::new(
            format!("Failed to create WS capability: {}", e),
            NanoServiceErrorStatus::InternalServerError,
        )
    })
}

pub fn verify_ws_capability(token: &str) -> Result<WsCapabilityClaims, NanoServiceError> {
    jsonwebtoken::decode::<WsCapabilityClaims>(
        token,
        &DecodingKey::from_secret(super::jwt::jwt_secret().as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| {
        NanoServiceError::new(
            format!("Invalid WS capability: {}", e),
            NanoServiceErrorStatus::Unauthorized,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_verify_ws_capability() {
        let user_id = Uuid::new_v4();
        let doc_id = Uuid::new_v4();
        let token = create_ws_capability(user_id, doc_id, false).unwrap();
        let claims = verify_ws_capability(&token).unwrap();
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.doc_id, doc_id);
        assert!(!claims.readonly);
    }

    #[test]
    fn rejects_malformed_ws_capability() {
        let err = verify_ws_capability("not-a-token").unwrap_err();
        assert_eq!(err.status, NanoServiceErrorStatus::Unauthorized);
    }
}
