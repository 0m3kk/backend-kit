pub use password_hasher::*;

#[cfg(feature = "async")]
use async_trait::async_trait;

/// No-op / Plaintext password hasher implementation for unit testing and benchmarking.
///
/// **WARNING**: DO NOT USE THIS HASHER IN PRODUCTION. It provides ZERO security!
#[derive(Debug, Clone, Default)]
pub struct NoopHasher;

impl NoopHasher {
    pub fn new() -> Self {
        Self
    }
}

impl PasswordHasher for NoopHasher {
    fn hash_password(&self, password: &str) -> Result<PasswordHash, PasswordError> {
        let formatted = format!("$noop${password}");
        Ok(PasswordHash::new(formatted, Algorithm::Noop))
    }

    fn verify_password(&self, password: &str, hash: &PasswordHash) -> Result<bool, PasswordError> {
        let expected = format!("$noop${password}");
        Ok(hash.as_str() == expected)
    }

    fn algorithm(&self) -> Algorithm {
        Algorithm::Noop
    }
}

#[cfg(feature = "async")]
#[async_trait]
impl AsyncPasswordHasher for NoopHasher {
    async fn hash_password_async(&self, password: String) -> Result<PasswordHash, PasswordError> {
        self.hash_password(&password)
    }

    async fn verify_password_async(
        &self,
        password: String,
        hash: PasswordHash,
    ) -> Result<bool, PasswordError> {
        self.verify_password(&password, &hash)
    }
}
