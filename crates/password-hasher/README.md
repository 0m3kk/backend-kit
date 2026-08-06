# password-hasher

Core traits, types, and errors for password hashing in backend-kit. Algorithm implementations live in separate crates:

| Crate                    | Description                       |
| :----------------------- | :-------------------------------- |
| `password-hasher`        | Core traits, types, errors        |
| `password-hasher-argon2` | Argon2id algorithm implementation |
| `password-hasher-bcrypt` | Bcrypt algorithm implementation   |

A no-op plaintext hasher for testing is also included behind the `noop` feature flag.

## Features

- **`PasswordHasher` trait**: Universal synchronous password hasher interface (`hash_password`, `verify_password`, `algorithm`).
- **`AsyncPasswordHasher` trait**: Async variant offloading CPU-heavy hashing to Tokio's `spawn_blocking` threadpool.
- **`PasswordHash`**: Strongly-typed wrapper with PHC string auto-detection (`$argon2id$`, `$2b$`, `$noop$`).
- **`Algorithm` enum**: `Argon2id`, `Bcrypt`, `Noop`.

## Feature Flags

| Feature | Description                                                  | Default |
| :------ | :----------------------------------------------------------- | :------ |
| `async` | Tokio `AsyncPasswordHasher` trait and spawn_blocking helpers | **Yes** |
| `noop`  | No-op plaintext `NoopHasher` for testing and benchmarking    | No      |

## Usage

Depend on an algorithm crate instead of the core crate directly — each implementation re-exports everything from `password-hasher`:

```toml
[dependencies]
password-hasher-argon2 = { path = "../password-hasher-argon2" }
password-hasher-bcrypt = { path = "../password-hasher-bcrypt" }
```

### Synchronous Password Hashing

```rust
use password_hasher::{PasswordHash, PasswordHasher};
use password_hasher_argon2::Argon2Hasher;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hasher = Argon2Hasher::new();

    // 1. Hash new password
    let password = "user_secret_password_123!";
    let hash = hasher.hash_password(password)?;
    println!("Generated Hash: {}", hash.as_str());

    // 2. Verify password
    let is_valid = hasher.verify_password(password, &hash)?;
    assert!(is_valid);

    // 3. Auto-detect algorithm from hash string
    let parsed = PasswordHash::parse(hash.as_str())?;
    assert_eq!(parsed.algorithm(), password_hasher::Algorithm::Argon2id);

    Ok(())
}
```

### Asynchronous Hashing (Tokio Background Thread Offloading)

```rust
use password_hasher::AsyncPasswordHasher;
use password_hasher_argon2::Argon2Hasher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hasher = Argon2Hasher::new();

    let password = "user_secret_password_123!".to_string();

    // Hashing runs safely on Tokio's blocking threadpool
    let hash = hasher.hash_password_async(password.clone()).await?;

    let is_valid = hasher.verify_password_async(password, hash).await?;
    assert!(is_valid);

    Ok(())
}
```
