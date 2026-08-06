use argon2::{
    Argon2, Params, Version,
    password_hash::{
        PasswordHash as RustCryptoPasswordHash, PasswordHasher as _, PasswordVerifier as _,
        SaltString,
    },
};
use rand_core::OsRng;

use crate::errors::PasswordError;
use crate::traits::PasswordHasher;
use crate::types::{Algorithm, PasswordHash};

#[cfg(feature = "async")]
use crate::traits::AsyncPasswordHasher;
#[cfg(feature = "async")]
use async_trait::async_trait;

/// Configuration parameters for Argon2id algorithm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Argon2Config {
    /// Memory size in KiB (default: 65536 KiB = 64 MiB)
    pub m_cost: u32,
    /// Number of passes / iterations (default: 3)
    pub t_cost: u32,
    /// Parallelism factor / degree of parallelism (default: 4)
    pub p_cost: u32,
    /// Desired key / hash output length in bytes (default: 32)
    pub output_len: usize,
}

impl Default for Argon2Config {
    fn default() -> Self {
        Self {
            m_cost: Params::DEFAULT_M_COST,
            t_cost: Params::DEFAULT_T_COST,
            p_cost: Params::DEFAULT_P_COST,
            output_len: Params::DEFAULT_OUTPUT_LEN,
        }
    }
}

/// Argon2id password hasher implementation.
#[derive(Debug, Clone, Default)]
pub struct Argon2Hasher {
    config: Argon2Config,
}

impl Argon2Hasher {
    /// Create a new `Argon2Hasher` with default OWASP parameters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new `Argon2Hasher` with custom parameters.
    pub fn with_config(config: Argon2Config) -> Self {
        Self { config }
    }
}

impl PasswordHasher for Argon2Hasher {
    fn hash_password(&self, password: &str) -> Result<PasswordHash, PasswordError> {
        let salt = SaltString::generate(&mut OsRng);
        let params = Params::new(
            self.config.m_cost,
            self.config.t_cost,
            self.config.p_cost,
            Some(self.config.output_len),
        )
        .map_err(|e| PasswordError::InvalidParameter(e.to_string()))?;

        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);

        let parsed_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| PasswordError::HashingError(e.to_string()))?;

        Ok(PasswordHash::new(
            parsed_hash.to_string(),
            Algorithm::Argon2id,
        ))
    }

    fn verify_password(&self, password: &str, hash: &PasswordHash) -> Result<bool, PasswordError> {
        let parsed_hash = RustCryptoPasswordHash::new(hash.as_str())
            .map_err(|e| PasswordError::InvalidFormat(e.to_string()))?;

        match Argon2::default().verify_password(password.as_bytes(), &parsed_hash) {
            Ok(()) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(e) => Err(PasswordError::HashingError(e.to_string())),
        }
    }

    fn algorithm(&self) -> Algorithm {
        Algorithm::Argon2id
    }
}

#[cfg(feature = "async")]
#[async_trait]
impl AsyncPasswordHasher for Argon2Hasher {
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
