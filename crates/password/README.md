# 🔒 password

Cryptographically secure password policy validation and Argon2id hashing for Rust applications.

---

## ✨ Features

- 🛡️ **Argon2id Algorithm**: State-of-the-art password hashing using the Argon2id key derivation function with cryptographically secure salt generation.
- 📋 **Configurable Password Policy**: Enforce length (min/max), character variety (uppercase, lowercase, numbers, special characters) via [`PasswordPolicy`].
- ⏱️ **Timing Attack Protection**: Constant-time verification through [`verify_password`].
- 📝 **Structured Errors**: Detailed validation failures with [`PasswordError`].

---

## 🚀 Quick Start

### 1. Add Dependency

Add `password` to your `Cargo.toml`:

```toml
[dependencies]
password = "0.1"
```

### 2. Usage Example

```rust
use password::{PasswordPolicy, hash_password, verify_password};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let policy = PasswordPolicy::default();
    let raw_password = "CorrectHorseBatteryStaple#9";

    // 1. Validate password policy
    policy.validate(raw_password)?;

    // 2. Hash password with Argon2id
    let hashed = hash_password(raw_password)?;
    println!("Argon2id Hash: {hashed}");

    // 3. Verify password
    let is_valid = verify_password(raw_password, &hashed)?;
    assert!(is_valid);

    Ok(())
}
```

---

## 📜 License

Licensed under the [MIT License](LICENSE).
