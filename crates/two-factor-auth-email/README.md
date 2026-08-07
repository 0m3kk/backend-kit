# two-factor-auth-email

Production-grade Email OTP (One-Time Password) 2FA backend for `backend-kit`.

## Overview

`two-factor-auth-email` provides secure Email passcode generation, `SecretStore` persistence with TTL expiration, constant-time verification, single-use consumption, and integration with the [`email-sender`](../email-sender) crate.

## Key Features

- **Pluggable Email Delivery**: Uses [`EmailSender`](../email-sender) to send 2FA passcodes via Resend, SendGrid, SMTP, or mock providers.
- **Envelope Encryption**: Persists user email enrollments and active passcodes securely using [`SecretStore`](../secret-store).
- **TTL Expiration & Replay Protection**: Passcodes automatically expire after TTL and are deleted immediately upon successful verification.
- **Generic 2FA Integration**: Implements `TwoFactorProvider` for unified multi-factor authentication workflows.

## Usage

Add `two-factor-auth-email` to your `Cargo.toml`:

```toml
[dependencies]
two-factor-auth-email = { path = "crates/two-factor-auth-email" }
```

```rust
use std::sync::Arc;
use email_sender::{EmailAddress, MemoryEmailSender};
use two_factor_auth_email::{EmailTwoFactorAuth, TwoFactorProvider, TwoFactorResponse};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let secret_store = Arc::new(/* ... SecretStore instance ... */);
    let email_sender = Arc::new(MemoryEmailSender::new());
    let sender_addr = EmailAddress::new("no-reply@example.com")?;

    let service = EmailTwoFactorAuth::new(secret_store, email_sender, sender_addr);

    // 1. Enroll user email
    service.enroll_user("alice", "alice@example.com").await?;

    // 2. Send Email code
    let masked_email = service.send_code("alice").await?;

    // 3. Verify user code
    let is_valid = service.verify_code("alice", "123456").await?;

    Ok(())
}
```
