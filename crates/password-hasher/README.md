# password-hasher

Universal password hashing library for backend-kit with support for Argon2id, Bcrypt, PBKDF2-SHA256, PHC format auto-detection, seamless algorithm migration, and async capabilities.

## Features

- **Argon2id (OWASP Recommended)**: Modern memory-hard password hashing algorithm backed by standard `argon2` crate.
- **Bcrypt**: Legacy and widespread password hashing algorithm support (`$2b$` standard).
- **PBKDF2-SHA256**: NIST-compliant password hashing algorithm support (`$pbkdf2-sha256$` standard).
- **No-op / Plaintext Hasher**: Fast, zero-computation hasher for unit testing and local benchmarking (`noop` feature).
- **PHC String Auto-Detection**: Standardized format parser (`$argon2id$`, `$2b$`, `$pbkdf2-sha256$`, `$noop$`) to automatically detect algorithms during verification.
- **`PasswordHasherManager`**: Universal router managing password hash verification, auto-dispatch, and detecting when existing hashes require re-hashing (`needs_rehash`) for seamless password security upgrades.
- **Async Threadpool Offloading**: Optional Tokio `spawn_blocking` integration (`AsyncPasswordHasher` trait) preventing CPU-heavy hashing loops from blocking async event loops.

## Feature Flags

| Feature  | Description                                                  | Default |
| :------- | :----------------------------------------------------------- | :------ |
| `argon2` | Argon2id password hasher support                             | **Yes** |
| `bcrypt` | Bcrypt password hasher support                               | **Yes** |
| `pbkdf2` | PBKDF2-HMAC-SHA256 password hasher support                   | No      |
| `async`  | Tokio `AsyncPasswordHasher` trait and spawn_blocking helpers | **Yes** |
| `noop`   | `NoopHasher` for mock testing                                | **Yes** |

## Code Example

### Synchronous Password Hashing & Auto-Verification

```rust
use password_hasher::{
    Argon2Hasher, BcryptHasher, PasswordHash, PasswordHasher, PasswordHasherManager,
};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create a PasswordHasherManager with Argon2id as default algorithm
    let manager = PasswordHasherManager::builder()
        .with_hasher(Arc::new(Argon2Hasher::new()))
        .with_hasher(Arc::new(BcryptHasher::new()))
        .build()?;

    // 2. Hash new password (uses Argon2id by default)
    let password = "user_secret_password_123!";
    let hash = manager.hash_password(password)?;
    println!("Generated Hash: {}", hash.as_str());

    // 3. Verify password (auto-detects algorithm from hash string)
    let is_valid = manager.verify_password(password, &hash)?;
    assert!(is_valid);

    // 4. Handle seamless hash upgrades (e.g. user logging in with old Bcrypt hash)
    let old_bcrypt_str = "$2b$12$e8Y7Yp3P9D/w/G5eH9T1eeG3Q1.2K8eG3Q1.2K8eG3Q1.2K8eG3Q1";
    if let Ok(old_hash) = PasswordHash::parse(old_bcrypt_str) {
        if manager.needs_rehash(&old_hash) {
            println!("Old hash format detected. Re-hashing password with default Argon2id...");
            let upgraded_hash = manager.hash_password(password)?;
            // Save upgraded_hash to database...
        }
    }

    Ok(())
}
```

### Asynchronous Hashing (Tokio Background Thread Offloading)

```rust
use password_hasher::{AsyncPasswordHasher, PasswordHasherManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = PasswordHasherManager::default();

    let password = "user_secret_password_123!".to_string();

    // Hashing runs safely on Tokio's blocking threadpool
    let hash = manager.hash_password_async(password.clone()).await?;

    let is_valid = manager.verify_password_async(password, hash).await?;
    assert!(is_valid);

    Ok(())
}
```
