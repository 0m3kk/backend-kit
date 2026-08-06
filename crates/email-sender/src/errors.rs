use thiserror::Error;

/// Error types encountered during email construction or transmission.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EmailError {
    /// Invalid or malformed email address format.
    #[error("Invalid email address: {0}")]
    InvalidAddress(String),

    /// Missing required recipient ('to', 'cc', or 'bcc').
    #[error("Email must have at least one recipient ('to', 'cc', or 'bcc')")]
    MissingRecipient,

    /// Missing required sender address ('from').
    #[error("Email must have a 'from' address")]
    MissingSender,

    /// Missing body content (neither text nor HTML body provided).
    #[error("Email must have either text or HTML content")]
    MissingContent,

    /// Invalid configuration for an email sender provider.
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// Transport or network communication error.
    #[error("Transport error: {0}")]
    TransportError(String),

    /// Provider API returned an error response.
    #[error("Provider '{provider}' error (status: {status_code:?}): {message}")]
    ProviderError {
        /// Name of the email service provider (e.g., "Resend", "SendGrid", "SMTP").
        provider: &'static str,
        /// HTTP status code or error code, if available.
        status_code: Option<u16>,
        /// Provider error message.
        message: String,
    },

    /// Serialization/deserialization failure.
    #[error("Serialization error: {0}")]
    SerializationError(String),
}
