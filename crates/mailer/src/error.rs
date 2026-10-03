use thiserror::Error;

/// Errors that can occur within the mailer service.
#[derive(Debug, Error)]
pub enum MailerError {
    /// Template engine or rendering error.
    #[error("Template error: {0}")]
    Template(String),

    /// Email message building error.
    #[error("Message build error: {0}")]
    MessageBuild(String),

    /// Transport / SMTP delivery error.
    #[error("Transport delivery error: {0}")]
    Transport(String),

    /// Configuration error.
    #[error("Mailer configuration error: {0}")]
    Config(String),

    /// IO error while loading templates.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
