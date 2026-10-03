# 🎫 jwt

Cryptographic JSON Web Token (JWT) generation, verification, and generic claims management for Rust applications.

---

## ✨ Features

- 🔑 **HMAC Signing & Verification**: Fast and secure token signing using `jsonwebtoken` with HMAC-SHA256 (`HS256`).
- 🧩 **Generic Claims (`Claims<T>`)**: Fully generic claims payload allowing any serializable custom struct `T` flattened alongside standard `sub`, `iat`, and `exp` claims.
- 🎯 **Arbitrary Payload Support**: Methods to encode and decode any serializable/deserializable struct directly without forcing specific claim envelopes.
- ⚙️ **Configurable Expiration**: Default token lifetimes via [`JwtConfig`].
- 🛡️ **Secret Redaction**: Debug printing automatically redacts sensitive signing secrets with [`JwtSecret`].
- ⏱️ **Validation Controls**: Supports custom `jsonwebtoken::Validation` rules (leeway, audiences, issuers).

---

## 🚀 Quick Start

### 1. Add Dependency

Add `jwt` to your `Cargo.toml`:

```toml
[dependencies]
jwt = "0.1"
serde = { version = "1.0", features = ["derive"] }
```

### 2. Standard Claims Example

```rust
use jwt::{JwtConfig, JwtService, Claims};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = JwtService::new("super-secret-signing-key-32-chars-long", JwtConfig::default());

    // 1. Generate standard token
    let token = service.generate_token("user-123")?;

    // 2. Verify and decode standard token
    let claims = service.verify_token::<()>(&token)?;
    println!("Subject: {}, Expiration: {}", claims.sub, claims.exp);

    Ok(())
}
```

### 3. Generic Custom Claims Example

```rust
use jwt::{JwtConfig, JwtService, Claims};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct UserPermissions {
    tenant_id: String,
    roles: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = JwtService::new("super-secret-signing-key-32-chars-long", JwtConfig::default());

    // 1. Create custom data
    let custom_data = UserPermissions {
        tenant_id: "tenant-acme".to_string(),
        roles: vec!["admin".to_string(), "billing".to_string()],
    };

    // 2. Generate token with custom claims
    let token = service.generate_token_with_custom("user-456", &custom_data)?;

    // 3. Verify and extract strongly-typed custom data
    let claims: Claims<UserPermissions> = service.verify_token(&token)?;
    println!("User: {}, Tenant: {}, Roles: {:?}", claims.sub, claims.custom.tenant_id, claims.custom.roles);

    Ok(())
}
```

---

## 📜 License

Licensed under the [MIT License](LICENSE).
