use crate::errors::PasswordError;
use crate::types::{Algorithm, PasswordHash};

/// Universal synchronous password hasher interface.
pub trait PasswordHasher: Send + Sync {
    /// Hash a plain-text password using the configured algorithm.
    fn hash_password(&self, password: &str) -> Result<PasswordHash, PasswordError>;

    /// Verify a plain-text password against a previously generated `PasswordHash`.
    fn verify_password(&self, password: &str, hash: &PasswordHash) -> Result<bool, PasswordError>;

    /// Return the algorithm supported by this hasher instance.
    fn algorithm(&self) -> Algorithm;
}

#[cfg(feature = "async")]
use async_trait::async_trait;

/// Asynchronous password hasher interface for CPU-bound hashing offloaded to threadpool workers.
#[cfg(feature = "async")]
#[async_trait]
pub trait AsyncPasswordHasher: PasswordHasher {
    /// Asynchronously hash a password offloading heavy computation to background threads.
    async fn hash_password_async(&self, password: String) -> Result<PasswordHash, PasswordError>;

    /// Asynchronously verify a password offloading heavy computation to background threads.
    async fn verify_password_async(
        &self,
        password: String,
        hash: PasswordHash,
    ) -> Result<bool, PasswordError>;
}
