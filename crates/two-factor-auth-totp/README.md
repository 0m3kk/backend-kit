# two-factor-auth-totp

Production-grade TOTP (Time-based One-Time Password) 2FA backend for `backend-kit` backed by the [`totp-rs`](https://crates.io/crates/totp-rs) engine.

## Overview

`two-factor-auth-totp` provides RFC 6238-compliant TOTP generation, token verification, `otpauth://` setup URI generation, and QR code rendering. It implements the generic `TwoFactorProvider` trait from `two-factor-auth`.

## Key Features

- **TOTP Service**: `TotpTwoFactorAuth` struct for secret creation, token calculation, and time-drift-tolerant verification.
- **Generic 2FA Integration**: Implements `TwoFactorProvider` for unified multi-factor authentication pipelines.
- **RFC 6238 Compliance**: Full support for SHA-1, SHA-256, SHA-512, 6/7/8 digits, custom time steps, and configurable clock skew windows.
- **QR Code Rendering**: Generate base64 Data URIs and raw PNG QR codes for enrollment with authenticator apps (Google Authenticator, Authy, 1Password).

## Usage

Add `two-factor-auth-totp` to your `Cargo.toml`:

```toml
[dependencies]
two-factor-auth-totp = { path = "crates/two-factor-auth-totp" }
```

### Direct TOTP Usage

```rust
use two_factor_auth_totp::{
    TotpAlgorithm, TotpConfig, TotpDigits, TotpTwoFactorAuth,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = TotpConfig::builder()
        .issuer("MyBackendService")
        .account_name("user@example.com")
        .algorithm(TotpAlgorithm::Sha1)
        .digits(TotpDigits::Six)
        .step(30)
        .build();

    let totp = TotpTwoFactorAuth::with_config(config);

    // 1. Generate new secret
    let secret = totp.generate_secret()?;

    // 2. Generate otpauth:// URI & QR code Data URI
    let url = totp.build_otpauth_url(&secret, totp.config())?;
    let qr_base64 = totp.generate_qr_base64(&secret)?;

    // 3. Verify user token
    let timestamp = 1700000000;
    let token = totp.generate_token(&secret, timestamp)?;
    let is_valid = totp.verify_token(&secret, &token, timestamp, 1)?;
    assert!(is_valid);

    Ok(())
}
```

### Generic 2FA Provider Usage

```rust
use two_factor_auth_totp::{
    TotpTwoFactorAuth, TwoFactorProvider, TwoFactorResponse, TwoFactorMethod,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = TotpTwoFactorAuth::new();
    assert_eq!(provider.method(), TwoFactorMethod::Totp);

    // Issue TOTP challenge (includes otpauth:// URI payload)
    let challenge = provider.issue_challenge("user_123").await?;

    // Verify response
    let response = TwoFactorResponse::totp("123456");
    let secret_b32 = "JBSWY3DPEHPK3PXP";
    let is_valid = provider.verify_response(secret_b32, &response).await?;

    Ok(())
}
```
