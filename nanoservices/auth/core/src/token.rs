use rand::Rng;
use utils::errors::NanoServiceError;

pub fn generate_verification_token() -> Result<String, NanoServiceError> {
    let token: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();
    Ok(token)
}

pub fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_verification_token() {
        let token = generate_verification_token().unwrap();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_generate_tokens_are_unique() {
        let token1 = generate_verification_token().unwrap();
        let token2 = generate_verification_token().unwrap();
        assert_ne!(token1, token2);
    }

    #[test]
    fn test_hash_token_is_deterministic() {
        let token = "test-token-value";
        let hash1 = hash_token(token);
        let hash2 = hash_token(token);
        assert_eq!(hash1, hash2);
    }
}
