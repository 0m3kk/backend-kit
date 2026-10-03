use thiserror::Error;

/// WebAuthn platform errors.
#[derive(Debug, Error)]
pub enum WebauthnError {
    /// Error originating from `webauthn-rs`.
    #[error("WebAuthn ceremony error: {0}")]
    Ceremony(#[from] webauthn_rs::prelude::WebauthnError),

    /// Invalid configuration.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Invalid user ID formatting for WebAuthn Uuid representation.
    #[error("Invalid user ID: {0}")]
    InvalidUserId(String),

    /// Serialization / Deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
