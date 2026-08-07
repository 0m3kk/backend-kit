# two-factor-auth-sms

Production-grade SMS OTP (One-Time Password) 2FA backend for `backend-kit`.

## Overview

`two-factor-auth-sms` provides secure SMS passcode generation, `SecretStore` persistence with TTL expiration, constant-time verification, single-use consumption, and an abstract [`SmsProvider`](src/lib.rs) trait for integrating with Twilio, AWS SNS, Vonage, or custom SMS gateways.

## Key Features

- **Pluggable SMS Delivery**: Implement [`SmsProvider`](src/lib.rs) to connect any SMS service gateway.
- **Built-in Mocking**: Includes [`MemorySmsProvider`](src/lib.rs) for instant local unit testing without external API credentials.
- **Envelope Encryption**: Persists user phone enrollments and active passcodes securely using [`SecretStore`](../secret-store).
- **TTL Expiration & Replay Protection**: Passcodes automatically expire after TTL and are deleted immediately upon successful verification.
- **Generic 2FA Integration**: Implements `TwoFactorProvider` for unified multi-factor authentication workflows.

## Usage

Add `two-factor-auth-sms` to your `Cargo.toml`:

```toml
[dependencies]
two-factor-auth-sms = { path = "crates/two-factor-auth-sms" }
```

```rust
use std::sync::Arc;
use two_factor_auth_sms::{MemorySmsProvider, SmsTwoFactorAuth, TwoFactorProvider, TwoFactorResponse};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let secret_store = Arc::new(/* ... SecretStore instance ... */);
    let sms_provider = Arc::new(MemorySmsProvider::new());

    let service = SmsTwoFactorAuth::new(secret_store, sms_provider);

    // 1. Enroll user phone
    service.enroll_user("alice", "+15551234567").await?;

    // 2. Send SMS code
    let masked_phone = service.send_code("alice").await?;

    // 3. Verify user code
    let is_valid = service.verify_code("alice", "123456").await?;

    Ok(())
}
```
