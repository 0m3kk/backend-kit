use crate::keyring::KeyVersion;

/// Error returned when a stored secret could not be decrypted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecryptError {
    /// The name of the secret that failed to decrypt.
    pub name: String,
}

/// Error returned by [`SecretStore`](crate::SecretStore).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error<E> {
    /// A stored secret could not be decrypted (wrong key, tampering, or a
    /// corrupted value). `name` identifies the entry, never the value.
    Decrypt(DecryptError),
    /// The key version of the stored secret was not found in the keyring.
    MissingKeyVersion {
        /// The name of the secret.
        name: String,
        /// The key version required.
        version: KeyVersion,
    },
    /// An error from the underlying store.
    Store(E),
}

impl<E> From<E> for Error<E> {
    fn from(error: E) -> Self {
        Self::Store(error)
    }
}
