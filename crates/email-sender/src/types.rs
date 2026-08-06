use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::errors::EmailError;

/// Represents an email address with an optional display name.
/// Example: `John Doe <john@example.com>` or `john@example.com`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EmailAddress {
    /// Optional recipient or sender human-readable display name.
    pub name: Option<String>,
    /// Raw email address string (e.g. "user@domain.com").
    pub email: String,
}

impl EmailAddress {
    /// Creates a new `EmailAddress` without a display name.
    pub fn new(email: impl Into<String>) -> Result<Self, EmailError> {
        let email_str = email.into();
        Self::validate_email(&email_str)?;
        Ok(Self {
            name: None,
            email: email_str,
        })
    }

    /// Creates a new `EmailAddress` with a display name and email address.
    pub fn with_name(
        name: impl Into<String>,
        email: impl Into<String>,
    ) -> Result<Self, EmailError> {
        let email_str = email.into();
        Self::validate_email(&email_str)?;
        let name_str = name.into();
        let trimmed_name = name_str.trim();
        let display_name = if trimmed_name.is_empty() {
            None
        } else {
            Some(trimmed_name.to_string())
        };

        Ok(Self {
            name: display_name,
            email: email_str,
        })
    }

    /// Basic email address validation logic.
    fn validate_email(email: &str) -> Result<(), EmailError> {
        let trimmed = email.trim();
        if trimmed.is_empty() {
            return Err(EmailError::InvalidAddress(
                "Email address cannot be empty".to_string(),
            ));
        }

        let parts: Vec<&str> = trimmed.split('@').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(EmailError::InvalidAddress(format!(
                "Invalid email format: '{trimmed}'"
            )));
        }

        if !parts[1].contains('.') {
            return Err(EmailError::InvalidAddress(format!(
                "Invalid email domain format: '{trimmed}'"
            )));
        }

        Ok(())
    }
}

impl fmt::Display for EmailAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.name {
            Some(name) => write!(f, "{name} <{}>", self.email),
            None => write!(f, "{}", self.email),
        }
    }
}

impl FromStr for EmailAddress {
    type Err = EmailError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if trimmed.contains('<') && trimmed.contains('>') {
            let start = match trimmed.find('<') {
                Some(idx) => idx,
                None => {
                    return Err(EmailError::InvalidAddress(format!(
                        "Invalid header format: '{s}'"
                    )));
                }
            };
            let end = match trimmed.rfind('>') {
                Some(idx) => idx,
                None => {
                    return Err(EmailError::InvalidAddress(format!(
                        "Invalid header format: '{s}'"
                    )));
                }
            };

            if start >= end {
                return Err(EmailError::InvalidAddress(format!(
                    "Invalid header format: '{s}'"
                )));
            }

            let name_part = trimmed[..start].trim().trim_matches('"');
            let email_part = &trimmed[start + 1..end];

            Self::with_name(name_part, email_part)
        } else {
            Self::new(trimmed)
        }
    }
}

/// Attachment data to be sent with an email.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    /// Filename of the attachment (e.g. "invoice.pdf").
    pub filename: String,
    /// Raw byte content of the attachment.
    pub content: Vec<u8>,
    /// MIME content type (e.g. "application/pdf", "image/png").
    pub content_type: String,
}

impl Attachment {
    /// Creates a new `Attachment`.
    pub fn new(
        filename: impl Into<String>,
        content: impl Into<Vec<u8>>,
        content_type: impl Into<String>,
    ) -> Self {
        Self {
            filename: filename.into(),
            content: content.into(),
            content_type: content_type.into(),
        }
    }
}

/// Representation of an email message ready to be sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Email {
    /// Sender email address.
    pub from: EmailAddress,
    /// Primary recipient email addresses ('To').
    pub to: Vec<EmailAddress>,
    /// Carbon copy recipient email addresses ('Cc').
    pub cc: Vec<EmailAddress>,
    /// Blind carbon copy recipient email addresses ('Bcc').
    pub bcc: Vec<EmailAddress>,
    /// Optional Reply-To email address.
    pub reply_to: Option<EmailAddress>,
    /// Subject line of the email.
    pub subject: String,
    /// Plain text body content.
    pub text_body: Option<String>,
    /// HTML body content.
    pub html_body: Option<String>,
    /// Attached files.
    pub attachments: Vec<Attachment>,
    /// Custom email headers.
    pub headers: HashMap<String, String>,
}

impl Email {
    /// Returns a new `EmailBuilder` to construct an `Email`.
    pub fn builder() -> EmailBuilder {
        EmailBuilder::default()
    }
}

/// Fluent builder for constructing [`Email`] instances.
#[derive(Debug, Default, Clone)]
pub struct EmailBuilder {
    from: Option<EmailAddress>,
    to: Vec<EmailAddress>,
    cc: Vec<EmailAddress>,
    bcc: Vec<EmailAddress>,
    reply_to: Option<EmailAddress>,
    subject: Option<String>,
    text_body: Option<String>,
    html_body: Option<String>,
    attachments: Vec<Attachment>,
    headers: HashMap<String, String>,
}

impl EmailBuilder {
    /// Creates a new, empty `EmailBuilder`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the sender ('From') email address.
    pub fn from(mut self, from: EmailAddress) -> Self {
        self.from = Some(from);
        self
    }

    /// Sets the sender ('From') email address from a string.
    pub fn from_str(self, from: &str) -> Result<Self, EmailError> {
        let address = EmailAddress::from_str(from)?;
        Ok(self.from(address))
    }

    /// Adds a primary recipient ('To').
    pub fn to(mut self, to: EmailAddress) -> Self {
        self.to.push(to);
        self
    }

    /// Adds a primary recipient ('To') from a string.
    pub fn to_str(self, to: &str) -> Result<Self, EmailError> {
        let address = EmailAddress::from_str(to)?;
        Ok(self.to(address))
    }

    /// Adds multiple primary recipients ('To').
    pub fn to_many(mut self, recipients: impl IntoIterator<Item = EmailAddress>) -> Self {
        self.to.extend(recipients);
        self
    }

    /// Adds a CC recipient.
    pub fn cc(mut self, cc: EmailAddress) -> Self {
        self.cc.push(cc);
        self
    }

    /// Adds a BCC recipient.
    pub fn bcc(mut self, bcc: EmailAddress) -> Self {
        self.bcc.push(bcc);
        self
    }

    /// Sets the Reply-To address.
    pub fn reply_to(mut self, reply_to: EmailAddress) -> Self {
        self.reply_to = Some(reply_to);
        self
    }

    /// Sets the email subject.
    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// Sets plain text body content.
    pub fn text_body(mut self, text: impl Into<String>) -> Self {
        self.text_body = Some(text.into());
        self
    }

    /// Sets HTML body content.
    pub fn html_body(mut self, html: impl Into<String>) -> Self {
        self.html_body = Some(html.into());
        self
    }

    /// Adds an attachment.
    pub fn attach(mut self, attachment: Attachment) -> Self {
        self.attachments.push(attachment);
        self
    }

    /// Adds a custom HTTP header.
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Builds and validates the [`Email`] instance.
    pub fn build(self) -> Result<Email, EmailError> {
        let from = self.from.ok_or(EmailError::MissingSender)?;

        if self.to.is_empty() && self.cc.is_empty() && self.bcc.is_empty() {
            return Err(EmailError::MissingRecipient);
        }

        if self.text_body.is_none() && self.html_body.is_none() {
            return Err(EmailError::MissingContent);
        }

        let subject = self.subject.unwrap_or_default();

        Ok(Email {
            from,
            to: self.to,
            cc: self.cc,
            bcc: self.bcc,
            reply_to: self.reply_to,
            subject,
            text_body: self.text_body,
            html_body: self.html_body,
            attachments: self.attachments,
            headers: self.headers,
        })
    }
}

/// Metadata response returned after sending an email.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailResponse {
    /// Provider message ID (e.g. SendGrid ID, Resend ID, SMTP Message-ID).
    pub message_id: Option<String>,
    /// Raw provider status or detail message.
    pub provider_response: Option<String>,
}

impl EmailResponse {
    /// Creates a new `EmailResponse`.
    pub fn new(message_id: Option<String>, provider_response: Option<String>) -> Self {
        Self {
            message_id,
            provider_response,
        }
    }
}
