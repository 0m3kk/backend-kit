# email-sender-sendgrid

SendGrid v3 API backend implementation of [`EmailSender`] for `backend-kit`.

## Features

- **SendGrid v3 Mail Send API Integration**: Directly sends email using SendGrid's `/v3/mail/send` REST endpoint.
- **Base64 Attachments**: Encodes file attachments to Base64 per SendGrid v3 specifications.
- **Headers & Personalizations**: Maps sender, primary recipients, CC, BCC, Reply-To, custom headers, and HTML/Text content.

## Code Example

```rust
use email_sender::{Email, EmailAddress, EmailSender};
use email_sender_sendgrid::{SendGridConfig, SendGridEmailSender};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = SendGridConfig::new("SG.your_api_key_here");
    let sender = SendGridEmailSender::new(config);

    let email = Email::builder()
        .from(EmailAddress::new("sender@example.com")?)
        .to(EmailAddress::new("user@example.com")?)
        .subject("SendGrid Email Test")
        .text_body("Hello from email-sender-sendgrid!")
        .build()?;

    let response = sender.send(&email).await?;
    println!("SendGrid response: {:?}", response);

    Ok(())
}
```
