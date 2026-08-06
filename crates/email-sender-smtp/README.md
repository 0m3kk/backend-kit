# email-sender-smtp

SMTP backend implementation of [`EmailSender`] using `lettre` for `backend-kit`.

## Features

- **SMTP Support**: Pure Rust SMTP client powered by `lettre`.
- **TLS Security**: Supports `None` (plain TCP), `StartTls`, and implicit `Tls`.
- **Full Features**: Supports HTML + plain text alternative parts, custom headers, multiple recipients (To, CC, BCC), and file attachments.

## Code Example

```rust
use email_sender::{Email, EmailAddress, EmailSender};
use email_sender_smtp::{SmtpConfig, SmtpCredentials, SmtpEmailSender, SmtpTlsMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = SmtpConfig::new("smtp.gmail.com")
        .port(587)
        .credentials(SmtpCredentials::new("user@gmail.com", "app_password"))
        .tls_mode(SmtpTlsMode::StartTls);

    let sender = SmtpEmailSender::new(&config)?;

    let email = Email::builder()
        .from(EmailAddress::new("user@gmail.com")?)
        .to(EmailAddress::new("friend@example.com")?)
        .subject("Hello via SMTP")
        .text_body("This email was sent via SmtpEmailSender!")
        .build()?;

    let response = sender.send(&email).await?;
    println!("Response: {:?}", response);

    Ok(())
}
```
