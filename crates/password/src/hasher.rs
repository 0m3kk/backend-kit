use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};

use crate::error::PasswordError;

/// Hashes a plaintext password using Argon2id with a cryptographically secure random salt.
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    let hash = Argon2::default()
        .hash_password(password.as_bytes())
        .map_err(|e| PasswordError::Hashing(e.to_string()))?;
    Ok(hash.to_string())
}

/// Verifies a plaintext password against an Argon2id password hash.
pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, PasswordError> {
    let parsed_hash =
        PasswordHash::new(password_hash).map_err(|e| PasswordError::Verification(e.to_string()))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_and_verifies_correctly() {
        #[allow(clippy::unwrap_used)]
        let hash = hash_password("ValidPassword123").unwrap();
        #[allow(clippy::unwrap_used)]
        let matches = verify_password("ValidPassword123", &hash).unwrap();
        assert!(matches);
        #[allow(clippy::unwrap_used)]
        let mismatches = verify_password("WrongPassword123", &hash).unwrap();
        assert!(!mismatches);
    }
}
