use serde::{Deserialize, Serialize};

/// Represents an outgoing email message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailMessage {
    /// Recipient email address.
    pub to: String,

    /// Recipient display name (optional).
    pub to_name: Option<String>,

    /// Email subject line.
    pub subject: String,

    /// HTML email body.
    pub html_body: String,

    /// Plain text email body (fallback).
    pub text_body: String,
}

impl EmailMessage {
    /// Creates a new email message.
    pub fn new(
        to: impl Into<String>,
        subject: impl Into<String>,
        html_body: impl Into<String>,
        text_body: impl Into<String>,
    ) -> Self {
        Self {
            to: to.into(),
            to_name: None,
            subject: subject.into(),
            html_body: html_body.into(),
            text_body: text_body.into(),
        }
    }

    /// Sets the recipient's display name.
    pub fn with_to_name(mut self, name: impl Into<String>) -> Self {
        self.to_name = Some(name.into());
        self
    }
}
