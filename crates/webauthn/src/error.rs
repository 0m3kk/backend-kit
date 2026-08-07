use thiserror::Error;

/// Error types for WebAuthn passkey registration, authentication, and management.
#[derive(Error, Debug)]
pub enum WebAuthnError {
    #[error("WebAuthn configuration error: {0}")]
    ConfigError(String),

    #[error("WebAuthn protocol error: {0}")]
    ProtocolError(String),

    #[error("Secret store error: {0}")]
    StoreError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("Passkey not found: {0}")]
    PasskeyNotFound(String),
}
