use thiserror::Error;

/// Core error types for Password Hasher operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PasswordError {
    #[error("Invalid password hash format: {0}")]
    InvalidFormat(String),

    #[error("Unsupported password hashing algorithm: {0}")]
    UnsupportedAlgorithm(String),

    #[error("Password verification failed")]
    VerificationFailed,

    #[error("Password hashing failed: {0}")]
    HashingError(String),

    #[error("Required feature is not enabled: {0}")]
    FeatureDisabled(&'static str),

    #[error("Async task execution error: {0}")]
    TaskError(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
}
