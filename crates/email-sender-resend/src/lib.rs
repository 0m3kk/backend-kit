use async_trait::async_trait;
use email_sender::{Email, EmailError, EmailResponse, EmailSender};
use resend_rs::Resend;
use resend_rs::types::{CreateAttachment, CreateEmailBaseOptions};

/// Configuration settings for the Resend API provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResendConfig {
    /// Resend API Key (e.g. `re_123456789`).
    pub api_key: String,
}

impl ResendConfig {
    /// Creates a new `ResendConfig`.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
        }
    }
}

/// Email sender using the [`resend_rs`] client SDK.
#[derive(Clone)]
pub struct ResendEmailSender {
    client: Resend,
    api_key: String,
}

impl ResendEmailSender {
    /// Creates a new [`ResendEmailSender`] with the provided [`ResendConfig`].
    pub fn new(config: ResendConfig) -> Self {
        let client = Resend::new(&config.api_key);
        Self {
            client,
            api_key: config.api_key,
        }
    }
}

#[async_trait]
impl EmailSender for ResendEmailSender {
    async fn send(&self, email: &Email) -> Result<EmailResponse, EmailError> {
        if self.api_key.trim().is_empty() {
            return Err(EmailError::ConfigurationError(
                "Resend API key is empty".to_string(),
            ));
        }

        let to: Vec<String> = email.to.iter().map(|addr| addr.to_string()).collect();

        let mut options = CreateEmailBaseOptions::new(email.from.to_string(), to, &email.subject);

        if let Some(text) = &email.text_body {
            options = options.with_text(text);
        }

        if let Some(html) = &email.html_body {
            options = options.with_html(html);
        }

        for cc in &email.cc {
            options = options.with_cc(&cc.to_string());
        }

        for bcc in &email.bcc {
            options = options.with_bcc(&bcc.to_string());
        }

        if let Some(reply_to) = &email.reply_to {
            options = options.with_reply(&reply_to.to_string());
        }

        if !email.attachments.is_empty() {
            let resend_atts: Vec<CreateAttachment> = email
                .attachments
                .iter()
                .map(|att| CreateAttachment::from(att.content.clone()).with_filename(&att.filename))
                .collect();
            options = options.with_attachments(resend_atts);
        }

        let response =
            self.client
                .emails
                .send(options)
                .await
                .map_err(|e| EmailError::ProviderError {
                    provider: "Resend",
                    status_code: None,
                    message: format!("{e}"),
                })?;

        Ok(EmailResponse::new(
            Some(response.id.to_string()),
            Some(format!("Resend email ID: {}", response.id)),
        ))
    }
}

#[cfg(test)]
mod tests {
    use email_sender::EmailAddress;

    use super::*;

    #[test]
    fn test_resend_config() {
        let config = ResendConfig::new("re_12345");
        assert_eq!(config.api_key, "re_12345");
    }

    #[tokio::test]
    async fn test_empty_api_key_error() -> Result<(), EmailError> {
        let sender = ResendEmailSender::new(ResendConfig::new(""));
        let email = Email::builder()
            .from(EmailAddress::new("from@test.com")?)
            .to(EmailAddress::new("to@test.com")?)
            .subject("Subject")
            .text_body("Body")
            .build()?;

        let res = sender.send(&email).await;
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            EmailError::ConfigurationError("Resend API key is empty".to_string())
        );
        Ok(())
    }
}
