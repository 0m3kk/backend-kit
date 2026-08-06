use password_hash::{
    PasswordHash as RustCryptoPasswordHash, PasswordHasher as _, PasswordVerifier as _, SaltString,
};
use pbkdf2::Pbkdf2;
use rand_core::OsRng;

use crate::errors::PasswordError;
use crate::traits::PasswordHasher;
use crate::types::{Algorithm, PasswordHash};

#[cfg(feature = "async")]
use crate::traits::AsyncPasswordHasher;
#[cfg(feature = "async")]
use async_trait::async_trait;

/// Configuration parameters for PBKDF2-SHA256 algorithm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pbkdf2Config {
    /// Number of iterations (OWASP recommendation for PBKDF2-HMAC-SHA256: 600,000)
    pub iterations: u32,
}

impl Default for Pbkdf2Config {
    fn default() -> Self {
        Self {
            iterations: 600_000,
        }
    }
}

/// PBKDF2-HMAC-SHA256 password hasher implementation.
#[derive(Debug, Clone, Default)]
pub struct Pbkdf2Hasher {
    _config: Pbkdf2Config,
}

impl Pbkdf2Hasher {
    /// Create a new `Pbkdf2Hasher` with default iteration count (600,000).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new `Pbkdf2Hasher` with custom configuration.
    pub fn with_config(config: Pbkdf2Config) -> Self {
        Self { _config: config }
    }
}

impl PasswordHasher for Pbkdf2Hasher {
    fn hash_password(&self, password: &str) -> Result<PasswordHash, PasswordError> {
        let salt = SaltString::generate(&mut OsRng);
        let parsed_hash = Pbkdf2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| PasswordError::HashingError(e.to_string()))?;

        Ok(PasswordHash::new(
            parsed_hash.to_string(),
            Algorithm::Pbkdf2Sha256,
        ))
    }

    fn verify_password(&self, password: &str, hash: &PasswordHash) -> Result<bool, PasswordError> {
        let parsed_hash = RustCryptoPasswordHash::new(hash.as_str())
            .map_err(|e| PasswordError::InvalidFormat(e.to_string()))?;

        match Pbkdf2.verify_password(password.as_bytes(), &parsed_hash) {
            Ok(()) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(e) => Err(PasswordError::HashingError(e.to_string())),
        }
    }

    fn algorithm(&self) -> Algorithm {
        Algorithm::Pbkdf2Sha256
    }
}

#[cfg(feature = "async")]
#[async_trait]
impl AsyncPasswordHasher for Pbkdf2Hasher {
    async fn hash_password_async(&self, password: String) -> Result<PasswordHash, PasswordError> {
        let hasher = self.clone();
        tokio::task::spawn_blocking(move || hasher.hash_password(&password))
            .await
            .map_err(|e| PasswordError::TaskError(e.to_string()))?
    }

    async fn verify_password_async(
        &self,
        password: String,
        hash: PasswordHash,
    ) -> Result<bool, PasswordError> {
        let hasher = self.clone();
        tokio::task::spawn_blocking(move || hasher.verify_password(&password, &hash))
            .await
            .map_err(|e| PasswordError::TaskError(e.to_string()))?
    }
}
