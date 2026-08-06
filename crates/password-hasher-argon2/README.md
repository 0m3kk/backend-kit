# password-hasher-argon2

Argon2id password hashing implementation of `PasswordHasher` and `AsyncPasswordHasher` for `backend-kit`.

## Features

- **Argon2id Algorithm**: Industry-standard memory-hard password hashing algorithm recommended by OWASP.
- **Configurable Parameters**: Custom memory cost (`m_cost`), time cost (`t_cost`), parallelism (`p_cost`), and hash output length.
- **OWASP Recommended Defaults**:
  - `m_cost`: 65536 KiB (64 MiB)
  - `t_cost`: 3 passes
  - `p_cost`: 4 threads
  - `output_len`: 32 bytes
- **Async Execution**: Offloads CPU-intensive computation to Tokio's `spawn_blocking` threadpool when the `async` feature is enabled.
- **Re-exports Core Traits**: Re-exports all core traits and types from `password-hasher`.

## Usage

Add `password-hasher-argon2` to your `Cargo.toml`:

```toml
[dependencies]
password-hasher-argon2 = { path = "../password-hasher-argon2" }
```

### Synchronous Usage

```rust
use password_hasher_argon2::{Argon2Hasher, PasswordHash, PasswordHasher};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hasher = Argon2Hasher::new();

    // 1. Hash password
    let password = "user_secret_password_123!";
    let hash = hasher.hash_password(password)?;
    println!("Generated Hash: {}", hash.as_str());

    // 2. Verify password
    let is_valid = hasher.verify_password(password, &hash)?;
    assert!(is_valid);

    // 3. Inspect algorithm
    assert_eq!(hasher.algorithm(), password_hasher_argon2::Algorithm::Argon2id);

    Ok(())
}
```

### Asynchronous Usage (Tokio Threadpool Offloading)

```rust
use password_hasher_argon2::{Argon2Hasher, AsyncPasswordHasher};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hasher = Argon2Hasher::new();
    let password = "user_secret_password_123!".to_string();

    // Hashing offloaded to Tokio spawn_blocking
    let hash = hasher.hash_password_async(password.clone()).await?;

    let is_valid = hasher.verify_password_async(password, hash).await?;
    assert!(is_valid);

    Ok(())
}
```
