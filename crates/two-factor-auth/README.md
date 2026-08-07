# two-factor-auth

Core generic traits, domain models, error definitions, and in-memory mock for Two-Factor Authentication (2FA) in `backend-kit`.

## Overview

`two-factor-auth` provides generic domain abstractions for multi-factor authentication (TOTP, SMS/Email OTP, and Backup Codes). It enables uniform challenge-response workflows across different 2FA mechanisms.

## Key Features

- **Generic Provider Trait**: [`TwoFactorProvider`](src/provider.rs) defining `issue_challenge`, `verify_response`, and `method()`.
- **2FA Primitives**: `TwoFactorMethod` (`Totp`, `SmsOtp`, `EmailOtp`, `BackupCode`), `TwoFactorChallenge`, `TwoFactorResponse`.
- **Recovery Utilities**: `BackupCode` generator & normalizer.
- **In-Memory Mocking**: `MemoryTwoFactorAuth` behind `memory` feature flag for unit testing.

## Usage

```rust
use two_factor_auth::{
    MemoryTwoFactorAuth, TwoFactorProvider, TwoFactorResponse, TwoFactorMethod,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = MemoryTwoFactorAuth::new();
    assert_eq!(service.method(), TwoFactorMethod::Totp);

    // 1. Issue a challenge
    let challenge = service.issue_challenge("alice@example.com").await?;

    // 2. Verify user response
    let response = TwoFactorResponse::totp("123456");
    let is_valid = service.verify_response(&challenge.challenge_id, &response).await?;
    assert!(is_valid);

    Ok(())
}
```
