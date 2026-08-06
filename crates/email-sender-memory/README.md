# email-sender-memory

In-memory mock [`EmailSender`] implementation for unit testing and local development.

## Features

- **Test Inspection**: Capture sent emails in an `Arc<RwLock<Vec<Email>>>`.
- **Fault Injection**: Simulate transport/provider errors dynamically via `set_forced_error(...)`.
- **Thread Safe**: Fully compatible with multi-threaded `tokio` unit tests.

## Code Example

```rust
use email_sender::{Email, EmailAddress, EmailSender};
use email_sender_memory::MemoryEmailSender;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sender = MemoryEmailSender::new();

    let email = Email::builder()
        .from(EmailAddress::new("no-reply@example.com")?)
        .to(EmailAddress::new("user@example.com")?)
        .subject("Test Email")
        .text_body("Testing in-memory email sender")
        .build()?;

    sender.send(&email).await?;

    assert_eq!(sender.count().await, 1);
    let last = sender.last_sent().await.unwrap();
    println!("Sent email with subject: {}", last.subject);

    Ok(())
}
```
