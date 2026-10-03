use async_trait::async_trait;

use crate::error::Error;
use crate::keyring::Keyring;
use crate::value::SecretBytes;

/// A secret store.
#[mockall::automock(type Error = std::io::Error;)]
#[async_trait]
pub trait SecretStore: Send + Sync {
    /// Error type of the underlying store.
    type Error: Send + Sync + 'static;

    /// Encrypt and store a secret under the name, using the active key in the keyring.
    async fn put(
        &self,
        keyring: &Keyring,
        name: &str,
        value: &SecretBytes,
    ) -> Result<(), Error<Self::Error>>;

    /// Fetch and decrypt the secret stored under the name, or `None` if the
    /// name does not exist.
    async fn get(
        &self,
        keyring: &Keyring,
        name: &str,
    ) -> Result<Option<SecretBytes>, Error<Self::Error>>;

    /// Delete the secret stored under the name, returning whether it existed.
    async fn delete(&self, name: &str) -> Result<bool, Error<Self::Error>>;

    /// List the names of all stored secrets. Never returns values.
    async fn list_names(&self) -> Result<Vec<String>, Error<Self::Error>>;

    /// Re-encrypt all stored secrets encrypted with older key versions to the
    /// active key version in the keyring, returning how many secrets were rotated.
    async fn rotate_keys(&self, keyring: &Keyring) -> Result<usize, Error<Self::Error>>;
}
