# email-sender

Core traits, domain models, error definitions, and fluent email builder for `backend-kit`.

## Features

- **Core Abstractions**: [`EmailSender`] trait for sending emails asynchronously.
- **Domain Models**: Structured representation of [`EmailAddress`], [`Attachment`], [`Email`], and [`EmailResponse`].
- **Fluent Builder**: [`EmailBuilder`] to construct and validate emails before transmission.
- **Robust Error Handling**: Explicit [`EmailError`] enum for validation failures, provider errors, and transport issues.

## Usage

```rust
use email_sender::{Email, EmailAddress, EmailError, EmailSender};

#[tokio::main]
async fn main() -> Result<(), EmailError> {
    let email = Email::builder()
        .from_str("Sender <sender@example.com>")?
        .to_str("Receiver <receiver@example.com>")?
        .subject("Welcome!")
        .text_body("Welcome to our platform.")
        .html_body("<h1>Welcome!</h1><p>Thanks for joining us.</p>")
        .build()?;

    println!("Created email for: {}", email.to[0]);
    Ok(())
}
```
