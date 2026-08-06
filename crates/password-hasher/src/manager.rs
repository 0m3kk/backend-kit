use std::collections::HashMap;
use std::sync::Arc;

use crate::errors::PasswordError;
use crate::traits::PasswordHasher;
use crate::types::{Algorithm, PasswordHash};

#[cfg(feature = "async")]
use crate::traits::AsyncPasswordHasher;
#[cfg(feature = "async")]
use async_trait::async_trait;

/// Unified password hasher manager providing automatic algorithm detection,
/// verification routing, and seamless password hash migration checks.
#[derive(Clone)]
pub struct PasswordHasherManager {
    default_algorithm: Algorithm,
    hashers: HashMap<Algorithm, Arc<dyn PasswordHasher>>,
}

impl PasswordHasherManager {
    /// Create a builder for configuring `PasswordHasherManager`.
    pub fn builder() -> PasswordHasherManagerBuilder {
        PasswordHasherManagerBuilder::default()
    }

    /// Return the current default algorithm.
    pub fn default_algorithm(&self) -> Algorithm {
        self.default_algorithm
    }

    /// Check if a given `PasswordHash` needs to be re-hashed using the default algorithm.
    pub fn needs_rehash(&self, hash: &PasswordHash) -> bool {
        hash.algorithm() != self.default_algorithm
    }

    /// Verify a password against a string hash representation with automatic algorithm detection.
    pub fn verify_password_str(
        &self,
        password: &str,
        hash_str: &str,
    ) -> Result<bool, PasswordError> {
        let hash = PasswordHash::parse(hash_str)?;
        self.verify_password(password, &hash)
    }

    /// Get a reference to the registered hasher for a specific algorithm.
    pub fn get_hasher(
        &self,
        algorithm: Algorithm,
    ) -> Result<&Arc<dyn PasswordHasher>, PasswordError> {
        self.hashers.get(&algorithm).ok_or_else(|| {
            PasswordError::UnsupportedAlgorithm(format!(
                "No hasher registered or enabled for algorithm {algorithm}"
            ))
        })
    }
}

impl PasswordHasher for PasswordHasherManager {
    fn hash_password(&self, password: &str) -> Result<PasswordHash, PasswordError> {
        let hasher = self.get_hasher(self.default_algorithm)?;
        hasher.hash_password(password)
    }

    fn verify_password(&self, password: &str, hash: &PasswordHash) -> Result<bool, PasswordError> {
        let hasher = self.get_hasher(hash.algorithm())?;
        hasher.verify_password(password, hash)
    }

    fn algorithm(&self) -> Algorithm {
        self.default_algorithm
    }
}

#[cfg(feature = "async")]
#[async_trait]
impl AsyncPasswordHasher for PasswordHasherManager {
    async fn hash_password_async(&self, password: String) -> Result<PasswordHash, PasswordError> {
        let manager = self.clone();
        tokio::task::spawn_blocking(move || manager.hash_password(&password))
            .await
            .map_err(|e| PasswordError::TaskError(e.to_string()))?
    }

    async fn verify_password_async(
        &self,
        password: String,
        hash: PasswordHash,
    ) -> Result<bool, PasswordError> {
        let manager = self.clone();
        tokio::task::spawn_blocking(move || manager.verify_password(&password, &hash))
            .await
            .map_err(|e| PasswordError::TaskError(e.to_string()))?
    }
}

/// Builder for `PasswordHasherManager`.
#[derive(Default)]
pub struct PasswordHasherManagerBuilder {
    default_algorithm: Option<Algorithm>,
    hashers: HashMap<Algorithm, Arc<dyn PasswordHasher>>,
}

impl PasswordHasherManagerBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn default_algorithm(mut self, algorithm: Algorithm) -> Self {
        self.default_algorithm = Some(algorithm);
        self
    }

    pub fn with_hasher(mut self, hasher: Arc<dyn PasswordHasher>) -> Self {
        self.hashers.insert(hasher.algorithm(), hasher);
        self
    }

    pub fn build(self) -> Result<PasswordHasherManager, PasswordError> {
        let default_algorithm = self
            .default_algorithm
            .or_else(|| self.hashers.keys().next().copied())
            .ok_or_else(|| {
                PasswordError::InvalidParameter("No hashers registered in manager".to_string())
            })?;

        if !self.hashers.contains_key(&default_algorithm) {
            return Err(PasswordError::InvalidParameter(format!(
                "Default algorithm {default_algorithm} is not registered in manager"
            )));
        }

        Ok(PasswordHasherManager {
            default_algorithm,
            hashers: self.hashers,
        })
    }
}
