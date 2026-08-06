# password-hasher

Core traits, types, and manager for password hashing in backend-kit. Algorithm implementations live in separate crates:

| Crate                    | Description                         |
| :----------------------- | :---------------------------------- |
| `password-hasher`        | Core traits, types, errors, manager |
| `password-hasher-argon2` | Argon2id algorithm implementation   |
| `password-hasher-bcrypt` | Bcrypt algorithm implementation     |
| `password-hasher-noop`   | No-op hasher for testing            |

## Features

- **`PasswordHasher` trait**: Universal synchronous password hasher interface (`hash_password`, `verify_password`, `algorithm`).
- **`AsyncPasswordHasher` trait**: Async variant offloading CPU-heavy hashing to Tokio's `spawn_blocking` threadpool.
- **`PasswordHasherManager`**: Multi-algorithm router with auto-detection from PHC hash format, `needs_rehash()` for seamless algorithm migration, and `verify_password_str()` for string-based verification.
- **`PasswordHash`**: Strongly-typed wrapper with PHC string auto-detection (`$argon2id$`, `$2b$`, `$noop$`).
- **`Algorithm` enum**: `Argon2id`, `Bcrypt`, `Noop`.

## Feature Flags

| Feature | Description                                                  | Default |
| :------ | :----------------------------------------------------------- | :------ |
| `async` | Tokio `AsyncPasswordHasher` trait and spawn_blocking helpers | **Yes** |

## Usage

Depend on an algorithm crate instead of the core crate directly — each implementation re-exports everything from `password-hasher`:

```toml
[dependencies]
password-hasher-argon2 = { path = "../password-hasher-argon2" }
password-hasher-bcrypt = { path = "../password-hasher-bcrypt" }
```

### Synchronous Password Hashing & Auto-Verification

```rust
use password_hasher::{PasswordHash, PasswordHasher, PasswordHasherManager};
use password_hasher_argon2::Argon2Hasher;
use password_hasher_bcrypt::BcryptHasher;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create a PasswordHasherManager with Argon2id as default algorithm
    let manager = PasswordHasherManager::builder()
        .with_hasher(Arc::new(Argon2Hasher::new()))
        .with_hasher(Arc::new(BcryptHasher::new()))
        .default_algorithm(password_hasher::Algorithm::Argon2id)
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
            println!("Old hash format detected. Re-hashing with default Argon2id...");
            let upgraded_hash = manager.hash_password(password)?;
            // Save upgraded_hash to database...
        }
    }

    Ok(())
}
```

### Asynchronous Hashing (Tokio Background Thread Offloading)

```rust
use password_hasher::{Algorithm, AsyncPasswordHasher, PasswordHasherManager};
use password_hasher_argon2::Argon2Hasher;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = PasswordHasherManager::builder()
        .with_hasher(Arc::new(Argon2Hasher::new()))
        .default_algorithm(Algorithm::Argon2id)
        .build()?;

    let password = "user_secret_password_123!".to_string();

    // Hashing runs safely on Tokio's blocking threadpool
    let hash = manager.hash_password_async(password.clone()).await?;

    let is_valid = manager.verify_password_async(password, hash).await?;
    assert!(is_valid);

    Ok(())
}
```
