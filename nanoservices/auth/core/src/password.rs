use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};

fn argon2() -> Argon2<'static> {
    // memory=64MB, iterations=3, parallelism=4 per architecture spec
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(65536, 3, 4, None).expect("valid argon2 params"),
    )
}

pub fn hash_password(password: &str) -> Result<String, NanoServiceError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = argon2();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| {
            NanoServiceError::new(
                format!("Failed to hash password: {}", e),
                NanoServiceErrorStatus::InternalServerError,
            )
        })?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, NanoServiceError> {
    let parsed_hash = PasswordHash::new(hash).map_err(|e| {
        NanoServiceError::new(
            format!("Invalid password hash: {}", e),
            NanoServiceErrorStatus::InternalServerError,
        )
    })?;
    Ok(argon2()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify_password() {
        let hash = hash_password("correct-password").unwrap();
        assert!(verify_password("correct-password", &hash).unwrap());
        assert!(!verify_password("wrong-password", &hash).unwrap());
    }

    #[test]
    fn test_hash_produces_different_outputs() {
        let hash1 = hash_password("same-password").unwrap();
        let hash2 = hash_password("same-password").unwrap();
        assert_ne!(hash1, hash2);
    }
}
