pub mod config;
pub mod error;
pub mod message;
pub mod sender;
pub mod service;
pub mod template;

pub use config::{MailerConfig, MailerProvider, MailerSecret, SmtpConfig};
pub use error::MailerError;
pub use message::EmailMessage;
pub use sender::{EmailSender, MockEmailSender, SmtpEmailSender};
pub use service::MailerService;
pub use template::TemplateEngine;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn renders_and_sends_templated_email() {
        let tmp = tempdir().unwrap();
        let reset_dir = tmp.path().join("reset_password");
        fs::create_dir_all(&reset_dir).unwrap();

        fs::write(
            reset_dir.join("subject.txt.jinja"),
            "Reset Password for {{ email }}",
        )
        .unwrap();

        fs::write(
            reset_dir.join("body.html.jinja"),
            "<p>Click <a href=\"{{ reset_url | safe }}\">here</a> to reset.</p>",
        )
        .unwrap();

        fs::write(
            reset_dir.join("body.txt.jinja"),
            "Go to {{ reset_url }} to reset password.",
        )
        .unwrap();

        let mock_sender = std::sync::Arc::new(MockEmailSender::new());
        let config = MailerConfig {
            templates_dir: tmp.path().to_str().unwrap().to_string(),
            ..Default::default()
        };

        let service = MailerService::new_with_sender(config, mock_sender.clone());

        let ctx = minijinja::context! {
            email => "test@example.com",
            reset_url => "https://example.com/reset?token=xyz",
        };

        service
            .send_templated("test@example.com", "reset_password", &ctx)
            .await
            .unwrap();

        let sent = mock_sender.sent_messages();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].to, "test@example.com");
        assert_eq!(sent[0].subject, "Reset Password for test@example.com");
        assert_eq!(
            sent[0].html_body,
            "<p>Click <a href=\"https://example.com/reset?token=xyz\">here</a> to reset.</p>"
        );
        assert_eq!(
            sent[0].text_body,
            "Go to https://example.com/reset?token=xyz to reset password."
        );
    }
}
