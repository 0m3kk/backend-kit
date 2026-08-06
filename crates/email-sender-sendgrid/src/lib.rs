use std::collections::HashMap;

use async_trait::async_trait;
use email_sender::{Email, EmailError, EmailResponse, EmailSender};
use sendgrid::v3::{
    Attachment as SendGridAttachment, Content, Email as SendGridAddress, Message, Personalization,
    Sender as SGClient,
};

/// Configuration settings for the SendGrid API provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendGridConfig {
    /// SendGrid API Key (e.g. `SG.1234...`).
    pub api_key: String,
}

impl SendGridConfig {
    /// Creates a new `SendGridConfig`.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
        }
    }
}

/// Email sender implementation using SendGrid client SDK crate.
#[derive(Clone)]
pub struct SendGridEmailSender {
    api_key: String,
}

impl SendGridEmailSender {
    /// Creates a new [`SendGridEmailSender`] with the given [`SendGridConfig`].
    pub fn new(config: SendGridConfig) -> Self {
        Self {
            api_key: config.api_key,
        }
    }
}

fn convert_address(addr: &email_sender::EmailAddress) -> SendGridAddress<'_> {
    let mut sg_addr = SendGridAddress::new(&addr.email);
    if let Some(name) = &addr.name {
        sg_addr = sg_addr.set_name(name);
    }
    sg_addr
}

#[async_trait]
impl EmailSender for SendGridEmailSender {
    async fn send(&self, email: &Email) -> Result<EmailResponse, EmailError> {
        if self.api_key.trim().is_empty() {
            return Err(EmailError::ConfigurationError(
                "SendGrid API key is empty".to_string(),
            ));
        }

        let client = SGClient::new(&self.api_key, None);

        let mut personalization = Personalization::new(convert_address(&email.from));

        for to in &email.to {
            personalization = personalization.add_to(convert_address(to));
        }

        for cc in &email.cc {
            personalization = personalization.add_cc(convert_address(cc));
        }

        for bcc in &email.bcc {
            personalization = personalization.add_bcc(convert_address(bcc));
        }

        if !email.headers.is_empty() {
            let mut sg_headers = HashMap::new();
            for (k, v) in &email.headers {
                sg_headers.insert(k.as_str(), v.as_str());
            }
            personalization = personalization.add_headers(&sg_headers);
        }

        let mut message = Message::new(convert_address(&email.from))
            .set_subject(&email.subject)
            .add_personalization(personalization);

        if let Some(reply_to) = &email.reply_to {
            message = message.set_reply_to(convert_address(reply_to));
        }

        if let Some(text) = &email.text_body {
            message = message.add_content(Content::new().set_value(text));
        }

        if let Some(html) = &email.html_body {
            message = message.add_content(Content::new().set_value(html));
        }

        for att in &email.attachments {
            let sg_att = SendGridAttachment::new()
                .set_filename(&att.filename)
                .set_content(&att.content);
            message = message.add_attachment(sg_att);
        }

        let response = client
            .send(&message)
            .await
            .map_err(|e| EmailError::ProviderError {
                provider: "SendGrid",
                status_code: None,
                message: format!("{e}"),
            })?;

        let status = response.status();
        let message_id = response
            .headers()
            .get("x-message-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let body_text = response.text().await.unwrap_or_default();

        Ok(EmailResponse::new(
            message_id,
            Some(if body_text.is_empty() {
                format!("SendGrid status: {status}")
            } else {
                body_text
            }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use email_sender::EmailAddress;

    use super::*;

    #[test]
    fn test_sendgrid_config() {
        let config = SendGridConfig::new("SG.12345");
        assert_eq!(config.api_key, "SG.12345");
    }

    #[tokio::test]
    async fn test_empty_api_key_error() -> Result<(), EmailError> {
        let sender = SendGridEmailSender::new(SendGridConfig::new(""));
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
            EmailError::ConfigurationError("SendGrid API key is empty".to_string())
        );
        Ok(())
    }
}
