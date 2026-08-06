use bcrypt::{DEFAULT_COST, hash, verify};

use crate::errors::PasswordError;
use crate::traits::PasswordHasher;
use crate::types::{Algorithm, PasswordHash};

#[cfg(feature = "async")]
use crate::traits::AsyncPasswordHasher;
#[cfg(feature = "async")]
use async_trait::async_trait;

/// Configuration parameters for Bcrypt algorithm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BcryptConfig {
    /// Cost factor (4 to 31, default: 12)
    pub cost: u32,
}

impl Default for BcryptConfig {
    fn default() -> Self {
        Self { cost: DEFAULT_COST }
    }
}

/// Bcrypt password hasher implementation.
#[derive(Debug, Clone, Default)]
pub struct BcryptHasher {
    config: BcryptConfig,
}

impl BcryptHasher {
    /// Create a new `BcryptHasher` with default cost parameter (12).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new `BcryptHasher` with custom cost parameter.
    pub fn with_config(config: BcryptConfig) -> Self {
        Self { config }
    }
}

impl PasswordHasher for BcryptHasher {
    fn hash_password(&self, password: &str) -> Result<PasswordHash, PasswordError> {
        let hashed_str = hash(password, self.config.cost)
            .map_err(|e| PasswordError::HashingError(e.to_string()))?;

        Ok(PasswordHash::new(hashed_str, Algorithm::Bcrypt))
    }

    fn verify_password(&self, password: &str, hash: &PasswordHash) -> Result<bool, PasswordError> {
        verify(password, hash.as_str()).map_err(|e| PasswordError::HashingError(e.to_string()))
    }

    fn algorithm(&self) -> Algorithm {
        Algorithm::Bcrypt
    }
}

#[cfg(feature = "async")]
#[async_trait]
impl AsyncPasswordHasher for BcryptHasher {
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
