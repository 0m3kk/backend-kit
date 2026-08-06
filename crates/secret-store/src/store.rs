use async_trait::async_trait;

use crate::errors::SecretError;
use crate::types::{
    ListSecretOptions, SecretEntry, SecretHeader, SecretPath, SecretValue, SetSecretOptions,
};

/// Universal Secret Store specification trait providing async secret management,
/// secret versioning, encrypted storage, tag filtering, master key rotation, and TTL expiration.
#[async_trait]
pub trait SecretStore: Send + Sync {
    /// Retrieve the latest active version of a secret by path.
    /// Returns `None` if the secret does not exist, is marked deleted, or has expired.
    async fn get(&self, path: &SecretPath) -> Result<Option<SecretEntry>, SecretError>;

    /// Retrieve a specific version of a secret by path and version number.
    /// Returns `None` if that specific version does not exist.
    async fn get_version(
        &self,
        path: &SecretPath,
        version: u64,
    ) -> Result<Option<SecretEntry>, SecretError>;

    /// Store a new version of a secret at path. Automatically increments secret version.
    async fn set(
        &self,
        path: SecretPath,
        value: SecretValue,
        options: SetSecretOptions,
    ) -> Result<SecretEntry, SecretError>;

    /// Delete a secret at path. Returns `true` if the secret was found and deleted.
    async fn delete(&self, path: &SecretPath) -> Result<bool, SecretError>;

    /// List secret headers matching prefix/tag filters without returning secret values.
    async fn list(&self, options: ListSecretOptions) -> Result<Vec<SecretHeader>, SecretError>;

    /// Rotate master key: re-encrypt stored secrets encrypted with `old_key_id` using `new_key_id`.
    /// Returns the number of secret versions re-encrypted.
    async fn rotate_key(&self, old_key_id: &str, new_key_id: &str) -> Result<u64, SecretError>;

    /// Purge up to `limit` expired secrets from the store. Returns the number of purged secret versions.
    async fn clean_expired(&self, limit: Option<usize>) -> Result<u64, SecretError>;
}
