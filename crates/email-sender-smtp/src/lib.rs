use async_trait::async_trait;
use email_sender::{Email, EmailError, EmailResponse, EmailSender};
use lettre::message::header::{ContentType, Header, HeaderName, HeaderValue};
use lettre::message::{Attachment as LettreAttachment, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};

/// TLS connection security mode for SMTP connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SmtpTlsMode {
    /// No TLS (plain TCP). Use primarily for local testing (e.g. MailHog / Mailpit).
    None,
    /// Upgrade to TLS via STARTTLS command (typically port 587).
    #[default]
    StartTls,
    /// Implicit TLS connection (typically port 465).
    Tls,
}

/// Authentication credentials for SMTP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmtpCredentials {
    /// SMTP login username.
    pub username: String,
    /// SMTP login password.
    pub password: String,
}

impl SmtpCredentials {
    /// Creates a new `SmtpCredentials`.
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }
}

/// Configuration settings for SMTP email sender.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmtpConfig {
    /// SMTP server hostname or IP address.
    pub host: String,
    /// SMTP server port (e.g. 587 for STARTTLS, 465 for TLS, 25 or 1025 for plain).
    pub port: u16,
    /// Optional authentication credentials.
    pub credentials: Option<SmtpCredentials>,
    /// TLS security mode.
    pub tls_mode: SmtpTlsMode,
}

impl SmtpConfig {
    /// Creates a new `SmtpConfig` with default settings (port 587, STARTTLS).
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            port: 587,
            credentials: None,
            tls_mode: SmtpTlsMode::StartTls,
        }
    }

    /// Sets the SMTP port.
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Sets authentication credentials.
    pub fn credentials(mut self, credentials: SmtpCredentials) -> Self {
        self.credentials = Some(credentials);
        self
    }

    /// Sets TLS security mode.
    pub fn tls_mode(mut self, mode: SmtpTlsMode) -> Self {
        self.tls_mode = mode;
        self
    }
}

/// Custom header type wrapper implementing `lettre::message::header::Header`.
#[derive(Debug, Clone)]
struct CustomRawHeader {
    name: HeaderName,
    value: String,
}

impl Header for CustomRawHeader {
    fn name() -> HeaderName {
        HeaderName::new_from_ascii_str("X-Custom-Header")
    }

    fn parse(_: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Err("parsing custom header unsupported".into())
    }

    fn display(&self) -> HeaderValue {
        HeaderValue::new(self.name.clone(), self.value.clone())
    }
}

/// SMTP email sender using `lettre`.
#[derive(Clone)]
pub struct SmtpEmailSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

impl SmtpEmailSender {
    /// Creates a new [`SmtpEmailSender`] from the given [`SmtpConfig`].
    pub fn new(config: &SmtpConfig) -> Result<Self, EmailError> {
        let builder = match config.tls_mode {
            SmtpTlsMode::None => {
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
            }
            SmtpTlsMode::StartTls => {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host).map_err(|e| {
                    EmailError::ConfigurationError(format!(
                        "Failed to configure STARTTLS relay: {e}"
                    ))
                })?
            }
            SmtpTlsMode::Tls => {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host).map_err(|e| {
                    EmailError::ConfigurationError(format!("Failed to configure TLS relay: {e}"))
                })?
            }
        };

        let builder = builder.port(config.port);

        let builder = if let Some(creds) = &config.credentials {
            builder.credentials(Credentials::new(
                creds.username.clone(),
                creds.password.clone(),
            ))
        } else {
            builder
        };

        let transport = builder.build();

        Ok(Self { transport })
    }

    /// Helper to convert our [`Email`] model into a [`lettre::Message`].
    pub fn build_lettre_message(email: &Email) -> Result<lettre::Message, EmailError> {
        let from_mailbox: lettre::message::Mailbox = email
            .from
            .to_string()
            .parse()
            .map_err(|e| EmailError::InvalidAddress(format!("Invalid 'from' address: {e}")))?;

        let mut builder = lettre::Message::builder().from(from_mailbox);

        for recipient in &email.to {
            let mailbox: lettre::message::Mailbox = recipient.to_string().parse().map_err(|e| {
                EmailError::InvalidAddress(format!("Invalid 'to' address '{recipient}': {e}"))
            })?;
            builder = builder.to(mailbox);
        }

        for recipient in &email.cc {
            let mailbox: lettre::message::Mailbox = recipient.to_string().parse().map_err(|e| {
                EmailError::InvalidAddress(format!("Invalid 'cc' address '{recipient}': {e}"))
            })?;
            builder = builder.cc(mailbox);
        }

        for recipient in &email.bcc {
            let mailbox: lettre::message::Mailbox = recipient.to_string().parse().map_err(|e| {
                EmailError::InvalidAddress(format!("Invalid 'bcc' address '{recipient}': {e}"))
            })?;
            builder = builder.bcc(mailbox);
        }

        if let Some(reply_to) = &email.reply_to {
            let mailbox: lettre::message::Mailbox = reply_to.to_string().parse().map_err(|e| {
                EmailError::InvalidAddress(format!("Invalid 'reply_to' address: {e}"))
            })?;
            builder = builder.reply_to(mailbox);
        }

        builder = builder.subject(&email.subject);

        for (header_key, header_val) in &email.headers {
            let name = HeaderName::new_from_ascii(header_key.clone()).map_err(|e| {
                EmailError::ConfigurationError(format!(
                    "Invalid custom header name '{header_key}': {e}"
                ))
            })?;
            builder = builder.header(CustomRawHeader {
                name,
                value: header_val.clone(),
            });
        }

        let body_part = match (&email.text_body, &email.html_body) {
            (Some(text), Some(html)) => MultiPart::alternative()
                .singlepart(SinglePart::plain(text.clone()))
                .singlepart(SinglePart::html(html.clone())),
            (Some(text), None) => {
                MultiPart::alternative().singlepart(SinglePart::plain(text.clone()))
            }
            (None, Some(html)) => {
                MultiPart::alternative().singlepart(SinglePart::html(html.clone()))
            }
            (None, None) => return Err(EmailError::MissingContent),
        };

        let message = if email.attachments.is_empty() {
            builder
                .multipart(body_part)
                .map_err(|e| EmailError::TransportError(format!("Failed to build message: {e}")))?
        } else {
            let mut mixed = MultiPart::mixed().multipart(body_part);

            for att in &email.attachments {
                let content_type = ContentType::parse(&att.content_type).map_err(|e| {
                    EmailError::ConfigurationError(format!(
                        "Invalid attachment content type '{}': {e}",
                        att.content_type
                    ))
                })?;

                let lettre_att = LettreAttachment::new(att.filename.clone())
                    .body(att.content.clone(), content_type);
                mixed = mixed.singlepart(lettre_att);
            }

            builder.multipart(mixed).map_err(|e| {
                EmailError::TransportError(format!("Failed to build multipart message: {e}"))
            })?
        };

        Ok(message)
    }
}

#[async_trait]
impl EmailSender for SmtpEmailSender {
    async fn send(&self, email: &Email) -> Result<EmailResponse, EmailError> {
        let lettre_msg = Self::build_lettre_message(email)?;

        let response = self
            .transport
            .send(lettre_msg)
            .await
            .map_err(|e| EmailError::TransportError(format!("SMTP send failed: {e}")))?;

        let message_id = response.first_line().map(|s| s.to_string());

        Ok(EmailResponse::new(
            message_id,
            Some(format!("SMTP Response code: {}", response.code())),
        ))
    }
}

#[cfg(test)]
mod tests {
    use email_sender::Attachment;

    use super::*;

    #[test]
    fn test_lettre_message_conversion() -> Result<(), EmailError> {
        let email = Email::builder()
            .from_str("Alice <alice@example.com>")?
            .to_str("Bob <bob@example.com>")?
            .subject("Hello Smtp")
            .text_body("Plain text")
            .html_body("<p>HTML text</p>")
            .header("X-Custom-Header", "CustomValue")
            .attach(Attachment::new("test.txt", b"hello".to_vec(), "text/plain"))
            .build()?;

        let lettre_msg = SmtpEmailSender::build_lettre_message(&email);
        assert!(lettre_msg.is_ok());
        Ok(())
    }

    #[test]
    fn test_smtp_config_builder() {
        let config = SmtpConfig::new("smtp.example.com")
            .port(465)
            .credentials(SmtpCredentials::new("user", "pass"))
            .tls_mode(SmtpTlsMode::Tls);

        assert_eq!(config.host, "smtp.example.com");
        assert_eq!(config.port, 465);
        assert_eq!(config.tls_mode, SmtpTlsMode::Tls);
        assert_eq!(
            config.credentials,
            Some(SmtpCredentials::new("user", "pass"))
        );
    }
}
