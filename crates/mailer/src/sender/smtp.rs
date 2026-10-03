use async_trait::async_trait;
use lettre::message::{Mailbox, MultiPart, SinglePart, header::ContentType};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use super::EmailSender;
use crate::config::{MailerSecret, SmtpConfig};
use crate::error::MailerError;
use crate::message::EmailMessage;

/// Production SMTP email sender backed by lettre.
pub struct SmtpEmailSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

impl SmtpEmailSender {
    /// Creates a new SMTP email sender.
    pub fn new(config: &SmtpConfig, secret: &MailerSecret) -> Result<Self, MailerError> {
        let mut builder = if config.tls {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
                .map_err(|e| {
                    MailerError::Config(format!("Invalid SMTP host {}: {}", config.host, e))
                })?
                .port(config.port)
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host).port(config.port)
        };

        if let (Some(user), Some(pass)) = (&secret.smtp_username, &secret.smtp_password) {
            builder = builder.credentials(Credentials::new(user.clone(), pass.clone()));
        }

        let transport = builder.build();
        Ok(Self { transport })
    }
}

#[async_trait]
impl EmailSender for SmtpEmailSender {
    async fn send(
        &self,
        from_name: &str,
        from_email: &str,
        message: &EmailMessage,
    ) -> Result<(), MailerError> {
        let from_header: Mailbox = format!("{from_name} <{from_email}>")
            .parse()
            .map_err(|e| MailerError::MessageBuild(format!("Invalid From header: {e}")))?;

        let to_header: Mailbox = if let Some(name) = &message.to_name {
            format!("{name} <{}>", message.to)
        } else {
            message.to.clone()
        }
        .parse()
        .map_err(|e| MailerError::MessageBuild(format!("Invalid To header: {e}")))?;

        let email = Message::builder()
            .from(from_header)
            .to(to_header)
            .subject(&message.subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(message.text_body.clone()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(message.html_body.clone()),
                    ),
            )
            .map_err(|e| MailerError::MessageBuild(e.to_string()))?;

        self.transport
            .send(email)
            .await
            .map_err(|e| MailerError::Transport(e.to_string()))?;

        Ok(())
    }
}
