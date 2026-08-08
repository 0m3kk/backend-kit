# webauthn

Production-grade WebAuthn / Passkeys primary passwordless authentication engine for `backend-kit`.

## Overview

`webauthn` provides RFC / W3C WebAuthn Level 2 & 3 passwordless authentication and passkey management built on top of [`webauthn-rs`](https://crates.io/crates/webauthn-rs) and integrated with [`SecretStore`](../secret-store) for envelope encryption of passkey credentials.

## Key Features

- **Passwordless Primary Authentication (1FA)**: Complete FIDO2 / Passkey registration and assertion flow (`navigator.credentials.create()` and `navigator.credentials.get()`).
- **WebAuthn Policy Engine**: Configure security requirements via `WebAuthnPolicy` and `WebAuthnPolicyBuilder` (user verification biometrics, platform vs. cross-platform authenticator attachment, discoverable passkeys, attestation conveyance).
- **SecretStore Integration**: Encrypts and stores user passkey credentials securely in `SecretStore`.
- **Replay Protection**: Validates passkey signature counters against credential clone attacks.
- **Passkey Lifecycle Management**: Easily list, query, and revoke passkeys per user.
- **Transactional API (`_tx` methods)**: `finish_registration_tx`, `start_authentication_tx`, `finish_authentication_tx`, `list_passkeys_tx`, and `delete_passkey_tx` accept an external `&mut Conn` via `SecretStoreTx<Conn>`, allowing passkey operations to participate in broader database transactions.

## Usage

Add `webauthn` to your `Cargo.toml`:

```toml
[dependencies]
webauthn = { path = "crates/webauthn" }
```

```rust
use std::sync::Arc;
use webauthn::{
    AuthenticatorAttachment, ResidentKeyRequirement, UserVerificationPolicy,
    WebAuthnAuthenticator, WebAuthnConfig, WebAuthnPolicy,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let secret_store = Arc::new(/* ... SecretStore instance ... */);

    // Configure WebAuthn Policy requirements
    let policy = WebAuthnPolicy::builder()
        .user_verification(UserVerificationPolicy::Required)
        .authenticator_attachment(AuthenticatorAttachment::Platform)
        .resident_key(ResidentKeyRequirement::Required)
        .build();

    let config = WebAuthnConfig::new(
        "example.com",
        "https://example.com",
        "My App Name",
    )
    .with_policy(policy);

    let authenticator = WebAuthnAuthenticator::new(secret_store, config)?;

    // 1. Start Passkey Registration
    let (challenge, reg_state) = authenticator
        .start_registration("user_123", "alice@example.com", "Alice")
        .await?;

    // Send `challenge` to frontend browser to invoke `navigator.credentials.create(...)`

    // 2. Start Passkey Authentication (Passwordless Login)
    let (auth_challenge, auth_state) = authenticator
        .start_authentication("user_123")
        .await?;

    // Send `auth_challenge` to frontend browser to invoke `navigator.credentials.get(...)`

    Ok(())
}
```
