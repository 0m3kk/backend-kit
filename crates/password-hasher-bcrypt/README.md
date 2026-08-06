# password-hasher-bcrypt

Bcrypt password hashing implementation of `PasswordHasher` and `AsyncPasswordHasher` for `backend-kit`.

## Features

- **Bcrypt Algorithm**: Widely used adaptive password hashing algorithm based on the Blowfish cipher.
- **Configurable Cost Factor**: Custom cost parameter (4 to 31, default: 12).
- **Async Execution**: Offloads CPU-intensive computation to Tokio's `spawn_blocking` threadpool when the `async` feature is enabled.
- **Re-exports Core Traits**: Re-exports all core traits and types from `password-hasher`.

## Usage

Add `password-hasher-bcrypt` to your `Cargo.toml`:

```toml
[dependencies]
password-hasher-bcrypt = { path = "../password-hasher-bcrypt" }
```

### Synchronous Usage

```rust
use password_hasher_bcrypt::{BcryptHasher, PasswordHash, PasswordHasher};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hasher = BcryptHasher::new();

    // 1. Hash password
    let password = "user_secret_password_123!";
    let hash = hasher.hash_password(password)?;
    println!("Generated Hash: {}", hash.as_str());

    // 2. Verify password
    let is_valid = hasher.verify_password(password, &hash)?;
    assert!(is_valid);

    // 3. Inspect algorithm
    assert_eq!(hasher.algorithm(), password_hasher_bcrypt::Algorithm::Bcrypt);

    Ok(())
}
```

### Asynchronous Usage (Tokio Threadpool Offloading)

```rust
use password_hasher_bcrypt::{AsyncPasswordHasher, BcryptHasher};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hasher = BcryptHasher::new();
    let password = "user_secret_password_123!".to_string();

    // Hashing offloaded to Tokio spawn_blocking
    let hash = hasher.hash_password_async(password.clone()).await?;

    let is_valid = hasher.verify_password_async(password, hash).await?;
    assert!(is_valid);

    Ok(())
}
```
