# 🛡️ webauthn

Production-ready WebAuthn and Passkeys authentication service, ceremonies, and configuration built on [`webauthn-rs`](https://github.com/kanidm/webauthn-rs).

---

## ✨ Features

- 🔑 **Passkeys & WebAuthn Ceremonies**: Full support for FIDO2 / WebAuthn registration and authentication ceremonies.
- 🏢 **Relying Party Config**: Flexible configuration for RP ID, RP Name, and Origin matching via [`WebauthnConfig`].
- 🛡️ **Customizable Security Policies**: Fine-grained [`WebauthnPolicy`] options:
  - User verification requirement (Required, Preferred, Discouraged).
  - Authenticator attachment (Platform / Roaming).
  - Resident key / discoverable credential preference.
  - Attestation conveyance preference.
  - Sign count validation to detect cloned authenticators.
- 💾 **State Serialization**: Safe serialization of registration and authentication ceremony states for stateless token exchange.
- 📑 **OpenAPI Integration**: OpenAPI schemas via `utoipa`.

---

## 🚀 Quick Start

### 1. Add Dependency

Add `webauthn` to your `Cargo.toml`:

```toml
[dependencies]
webauthn = "0.1"
```

### 2. Usage Example

```rust
use webauthn::{
    WebauthnConfig, WebauthnPolicy, WebauthnService,
};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize config and policy
    let config = WebauthnConfig::default();
    let policy = WebauthnPolicy::default();

    // 2. Instantiate WebAuthn service
    let service = WebauthnService::new(&config, policy)?;

    // 3. Start registration ceremony
    let user_id = Uuid::now_v7();
    let (creation_challenge, reg_state) = service
        .start_registration(user_id, "user@example.com", "Alice", &[])?;

    println!("Send challenge to client: {:?}", creation_challenge);

    Ok(())
}
```

---

## 📜 License

Licensed under the [MIT License](LICENSE).
