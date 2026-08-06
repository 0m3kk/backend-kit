# email-sender-resend

Resend API backend implementation of [`EmailSender`] for `backend-kit`.

## Features

- **Resend REST API Integration**: Directly sends email using Resend's API (`/emails`).
- **Base64 Attachments**: Automatically encodes email attachments to base64 for Resend payload.
- **Custom Headers & Metadata**: Supports custom HTTP headers, reply-to, CC, BCC, and HTML/Text content.

## Code Example

```rust
use email_sender::{Email, EmailAddress, EmailSender};
use email_sender_resend::{ResendConfig, ResendEmailSender};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ResendConfig::new("re_123456789");
    let sender = ResendEmailSender::new(config);

    let email = Email::builder()
        .from(EmailAddress::new("onboarding@resend.dev")?)
        .to(EmailAddress::new("user@example.com")?)
        .subject("Hello from Resend")
        .html_body("<h1>Welcome!</h1><p>Sent using email-sender-resend.</p>")
        .build()?;

    let response = sender.send(&email).await?;
    println!("Resend Message ID: {:?}", response.message_id);

    Ok(())
}
```
