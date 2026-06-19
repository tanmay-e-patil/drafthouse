use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use kernel::JwtClaims;
use std::env;
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

pub(crate) fn jwt_secret() -> String {
    env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-change-in-production".into())
}

fn jwt_expiry_secs() -> usize {
    env::var("JWT_EXPIRY_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(900)
}

pub fn create_jwt(
    user_id: uuid::Uuid,
    email: &str,
    verified: bool,
) -> Result<String, NanoServiceError> {
    let now = Utc::now();
    let iat = now.timestamp() as usize;
    let exp = iat + jwt_expiry_secs();

    let claims = JwtClaims {
        sub: user_id,
        email: email.to_string(),
        verified,
        exp,
        iat,
    };

    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret().as_bytes()),
    )
    .map_err(|e| {
        NanoServiceError::new(
            format!("Failed to create JWT: {}", e),
            NanoServiceErrorStatus::InternalServerError,
        )
    })
}

pub fn verify_jwt(token: &str) -> Result<JwtClaims, NanoServiceError> {
    let validation = Validation::default();
    jsonwebtoken::decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret().as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|e| {
        let status = match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                NanoServiceErrorStatus::Unauthorized
            }
            _ => NanoServiceErrorStatus::Unauthorized,
        };
        NanoServiceError::new(format!("Invalid JWT: {}", e), status)
    })
}

pub fn require_verified(claims: &JwtClaims) -> Result<(), NanoServiceError> {
    if claims.verified {
        Ok(())
    } else {
        Err(NanoServiceError::new(
            "Email verification required",
            NanoServiceErrorStatus::Forbidden,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_create_and_verify_jwt() {
        let user_id = Uuid::new_v4();
        let token = create_jwt(user_id, "test@example.com", true).unwrap();
        let claims = verify_jwt(&token).unwrap();
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.email, "test@example.com");
        assert!(claims.verified);
    }

    #[test]
    fn test_unverified_jwt() {
        let user_id = Uuid::new_v4();
        let token = create_jwt(user_id, "test@example.com", false).unwrap();
        let claims = verify_jwt(&token).unwrap();
        assert!(!claims.verified);
    }

    #[test]
    fn test_require_verified_allows_verified() {
        let user_id = Uuid::new_v4();
        let token = create_jwt(user_id, "test@example.com", true).unwrap();
        let claims = verify_jwt(&token).unwrap();
        assert!(require_verified(&claims).is_ok());
    }

    #[test]
    fn test_require_verified_rejects_unverified() {
        let user_id = Uuid::new_v4();
        let token = create_jwt(user_id, "test@example.com", false).unwrap();
        let claims = verify_jwt(&token).unwrap();
        assert!(require_verified(&claims).is_err());
    }
}
