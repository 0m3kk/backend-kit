use std::path::Path;
use std::sync::Arc;

use crate::config::{MailerConfig, MailerProvider, MailerSecret};
use crate::error::MailerError;
use crate::message::EmailMessage;
use crate::sender::{EmailSender, MockEmailSender, SmtpEmailSender};
use crate::template::TemplateEngine;

/// High-level mailer service managing templates and email transport.
#[derive(Clone)]
pub struct MailerService {
    config: MailerConfig,
    template_engine: TemplateEngine,
    sender: Arc<dyn EmailSender>,
}

impl MailerService {
    /// Creates a new MailerService based on configuration and secrets.
    pub fn new(config: MailerConfig, secret: MailerSecret) -> Result<Self, MailerError> {
        let template_engine = TemplateEngine::new(&config.templates_dir);

        let sender: Arc<dyn EmailSender> = match config.provider {
            MailerProvider::Smtp => Arc::new(SmtpEmailSender::new(&config.smtp, &secret)?),
            MailerProvider::Mock => Arc::new(MockEmailSender::new()),
        };

        Ok(Self {
            config,
            template_engine,
            sender,
        })
    }

    /// Creates a MailerService with a custom sender implementation (useful for tests).
    pub fn new_with_sender(config: MailerConfig, sender: Arc<dyn EmailSender>) -> Self {
        let template_engine = TemplateEngine::new(&config.templates_dir);
        Self {
            config,
            template_engine,
            sender,
        }
    }

    /// Accessor for mailer configuration.
    pub fn config(&self) -> &MailerConfig {
        &self.config
    }

    /// Accessor for the template directory path.
    pub fn templates_dir(&self) -> &Path {
        self.template_engine.templates_dir()
    }

    /// Accessor for the underlying sender.
    pub fn sender(&self) -> &Arc<dyn EmailSender> {
        &self.sender
    }

    /// Renders a single template by file path relative to `templates_dir`.
    pub fn render_template<S: serde::Serialize>(
        &self,
        template_name: &str,
        ctx: &S,
    ) -> Result<String, MailerError> {
        self.template_engine.render(template_name, ctx)
    }

    /// Sends an `EmailMessage` using the configured sender.
    pub async fn send(&self, message: &EmailMessage) -> Result<(), MailerError> {
        self.sender
            .send(&self.config.from_name, &self.config.from_email, message)
            .await
    }

    /// Convenience method to build and send an email directly.
    pub async fn send_to(
        &self,
        to: impl Into<String>,
        subject: impl Into<String>,
        html_body: impl Into<String>,
        text_body: impl Into<String>,
    ) -> Result<(), MailerError> {
        let message = EmailMessage::new(to, subject, html_body, text_body);
        self.send(&message).await
    }

    /// Renders `{template_name}/subject.txt.jinja`, `{template_name}/body.html.jinja`,
    /// and `{template_name}/body.txt.jinja` with the given context, then dispatches the email.
    pub async fn send_templated<S: serde::Serialize>(
        &self,
        to: impl Into<String>,
        template_name: &str,
        ctx: &S,
    ) -> Result<(), MailerError> {
        let subject_file = format!("{template_name}/subject.txt.jinja");
        let html_file = format!("{template_name}/body.html.jinja");
        let text_file = format!("{template_name}/body.txt.jinja");

        let subject = self.render_template(&subject_file, ctx)?;
        let html_body = self.render_template(&html_file, ctx)?;
        let text_body = self.render_template(&text_file, ctx)?;

        // Trim subject to avoid trailing newlines
        let message = EmailMessage::new(to, subject.trim(), html_body, text_body);
        self.send(&message).await
    }
}
