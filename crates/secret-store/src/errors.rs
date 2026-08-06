use thiserror::Error;

/// Core error types for Secret Store operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecretError {
    #[error("Secret not found: {path}")]
    SecretNotFound { path: String },

    #[error("Secret version {version} not found for path: {path}")]
    VersionNotFound { path: String, version: u64 },

    #[error("Invalid secret path: {0}")]
    InvalidPath(String),

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Decryption error: {0}")]
    DecryptionError(String),

    #[error("Key provider error: {0}")]
    KeyProviderError(String),

    #[error("Secret has expired: {path}")]
    Expired { path: String },

    #[error("Precondition failed: {0}")]
    ConditionFailed(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Secret store error: {0}")]
    StoreError(String),
}
