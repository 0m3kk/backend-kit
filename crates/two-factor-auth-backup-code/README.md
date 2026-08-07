# two-factor-auth-backup-code

Production-grade Backup / Recovery Code 2FA backend for `backend-kit`.

## Overview

`two-factor-auth-backup-code` provides secure generation, SHA-256 hashing, verification, and single-use consumption of recovery backup codes. It implements the generic `TwoFactorProvider` trait from `two-factor-auth`.

## Key Features

- **Secure Hashing**: Hashes backup codes with SHA-256 before storage so raw codes are never saved in database tables.
- **Single-Use Consumption**: `verify_and_consume` invalidates used backup codes and returns the updated set of remaining active code hashes.
- **Generic 2FA Integration**: Implements `TwoFactorProvider` for unified multi-factor authentication pipelines.
- **Format Normalization**: Automatically handles whitespace, dashes, and casing during code entry (e.g. `a1b2-c3d4` vs `A1B2C3D4`).

## Usage

Add `two-factor-auth-backup-code` to your `Cargo.toml`:

```toml
[dependencies]
two-factor-auth-backup-code = { path = "crates/two-factor-auth-backup-code" }
```

```rust
use two_factor_auth_backup_code::{BackupCodeTwoFactorAuth, TwoFactorProvider, TwoFactorResponse};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = BackupCodeTwoFactorAuth::new();

    // 1. Generate 8 backup codes for user enrollment
    let set = service.generate_codes(8);
    // set.plain_codes -> Display once to user
    // set.hashed_codes -> Save to database

    // 2. User submits a backup code during login
    let submitted_code = &set.plain_codes[0];
    let remaining = service.verify_and_consume(submitted_code, &set.hashed_codes)?;
    assert!(remaining.is_some());

    // remaining.unwrap() contains the 7 remaining active hashed codes
    Ok(())
}
```
