# 📧 mailer

Production-ready email delivery service abstraction, SMTP transport, and MiniJinja template rendering for Rust applications.

---

## ✨ Features

- ✉️ **Transport Abstraction**: Clean [`EmailSender`] trait supporting both live [`SmtpEmailSender`] and testing [`MockEmailSender`].
- 🎨 **MiniJinja Templates**: Fast, secure HTML and plain text email rendering from configurable template directories using [`TemplateEngine`].
- 🔒 **TLS & Authentication**: Configurable SMTP TLS modes (None, STARTTLS, Wrapper) and credentials with [`MailerSecret`].
- 📝 **Structured Errors**: Clear and strongly-typed errors with [`MailerError`].
- 🧪 **Mocking Ready**: [`MockEmailSender`] for reliable unit and integration testing without sending actual emails.

---

## 🚀 Quick Start

### 1. Add Dependency

Add `mailer` to your `Cargo.toml`:

```toml
[dependencies]
mailer = "0.1"
```

### 2. Usage Example

```rust
use mailer::{
    EmailMessage, MailerConfig, MailerService, SmtpConfig, SmtpEmailSender,
    TemplateEngine,
};
use serde_json::json;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize template engine pointing to templates folder
    let template_engine = TemplateEngine::new("rsc/templates")?;

    // 2. Setup SMTP sender
    let smtp_config = SmtpConfig::default();
    let sender = Arc::new(SmtpEmailSender::new(&smtp_config, None)?);

    // 3. Create mailer service
    let service = MailerService::new(sender, template_engine, "no-reply@example.com");

    // 4. Render and send an email
    let context = json!({
        "username": "Alice",
        "action_url": "https://example.com/verify?token=xyz",
    });

    service
        .send_templated(
            "user@example.com",
            "verify_email",
            context,
        )
        .await?;

    println!("Email sent successfully!");
    Ok(())
}
```

---

## 🧪 Testing with Mock

Use [`MockEmailSender`] in tests:

```rust
use mailer::{EmailMessage, MockEmailSender, EmailSender};

#[tokio::test]
async fn test_mock_email_sender() {
    let mock = MockEmailSender::new();
    let msg = EmailMessage::new("from@example.com", "to@example.com", "Hello", "Body");

    mock.send(msg).await.unwrap();

    let sent = mock.sent_messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].subject, "Hello");
}
```

---

## 📜 License

Licensed under the [MIT License](LICENSE).
