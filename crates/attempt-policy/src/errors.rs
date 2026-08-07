use thiserror::Error;

/// Error type for attempt policy tracking and lockouts.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AttemptError {
    /// Maximum attempts exceeded; operation is locked out.
    #[error("Maximum attempts exceeded. Try again in {retry_after_secs} seconds")]
    MaxAttemptsExceeded { retry_after_secs: u64 },

    /// Storage or KV store backend operation failed.
    #[error("Attempt tracker storage error: {0}")]
    StorageError(String),

    /// Invalid configuration parameters.
    #[error("Invalid attempt policy configuration: {0}")]
    InvalidConfiguration(String),
}
