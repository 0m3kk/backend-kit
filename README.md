# Backend Kit

A production-grade, modular collection of foundational Rust backend libraries and infrastructure components designed for reliability, type safety, and testability.

---

## Overview & Architecture

`backend-kit` is organized as a Cargo workspace providing decoupled abstractions and pluggable backends for common backend requirements:

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│                                   Backend Kit                                    │
└──────────────────────────────────────────────────────────────────────────────────┘
├── Event Sourcing & CQRS ──► event-sourcing, event-store-postgres, event-store-umadb, cqrs
├── Key-Value Storage     ──► kv-store, kv-store-postgres, kv-store-redb, kv-store-redis
├── Secret Management     ──► secret-store, secret-store-postgres
├── Password Hashing      ──► password-hasher, password-hasher-argon2, password-hasher-bcrypt
├── Password Policy       ──► password-policy
├── Attempt Policy        ──► attempt-policy
├── Email Delivery        ──► email-sender, email-sender-resend, email-sender-sendgrid, email-sender-smtp
├── SMS Delivery          ──► sms-sender
├── Template Engines      ──► template-engine, template-engine-askama, template-engine-tera
├── WebAuthn / Passkeys   ──► webauthn
└── Two-Factor Auth       ──► two-factor-auth, two-factor-auth-totp, two-factor-auth-backup-code, two-factor-auth-sms, two-factor-auth-email
```

### Key Design Principles

1. **Clean Architecture & Trait Abstraction**: Core crates (`kv-store`, `event-sourcing`, `secret-store`, `email-sender`, `sms-sender`, `password-hasher`, `template-engine`, `two-factor-auth`, `webauthn`, `attempt-policy`) define pure async trait interfaces and domain models without heavy infrastructure dependencies.
2. **Pluggable Implementations**: Swap backends seamlessly (e.g., PostgreSQL vs. Redis vs. Redb, or Resend vs. SendGrid vs. SMTP) by changing dependencies without mutating business logic.
3. **Built-in Mocking for Unit Testing**: Core crates provide optional in-memory stores (`memory` feature flag) and no-op implementations (`noop` feature flag) for instant unit testing without external database containers.
4. **Strict Quality Standards**: Workspace-wide Clippy configuration (`unwrap_used = "deny"`, `expect_used = "deny"`) ensuring production runtime safety.

---

## Workspace Crates Index

| Module Domain           | Crate                                                                         | Path                                                                                 | Description                                                                                                                                                                                                                                                |
| :---------------------- | :---------------------------------------------------------------------------- | :----------------------------------------------------------------------------------- | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Attempt Policy**      | [`attempt-policy`](crates/attempt-policy/README.md)                           | [`crates/attempt-policy`](crates/attempt-policy/README.md)                           | Core attempt tracking, max attempt policy enforcement (`AttemptPolicy`), sliding window rate-limiting, exponential lockouts, `InMemoryAttemptTracker`, and pluggable `KvAttemptTracker` backend.                                                           |
| **CQRS**                | [`cqrs`](crates/cqrs/README.md)                                               | [`crates/cqrs`](crates/cqrs/README.md)                                               | 5-step Command pipeline (`Command<M, C>`), multi-model hydration (`DecisionModels`), KV snapshots (`dispatch_command_with_snapshot`), object-safe views (`View<C>`), checkpoints (`KvCheckpointStore`), and parallel background workers (`CatchupWorker`). |
| **Email**               | [`email-sender`](crates/email-sender/README.md)                               | [`crates/email-sender`](crates/email-sender/README.md)                               | Core traits, domain models (`Email`, `EmailAddress`, `Attachment`), fluent builder (`EmailBuilder`), and error definitions. Optional `MemoryEmailSender` behind `memory` feature.                                                                          |
|                         | [`email-sender-resend`](crates/email-sender-resend/README.md)                 | [`crates/email-sender-resend`](crates/email-sender-resend/README.md)                 | Resend REST API integration backend.                                                                                                                                                                                                                       |
|                         | [`email-sender-sendgrid`](crates/email-sender-sendgrid/README.md)             | [`crates/email-sender-sendgrid`](crates/email-sender-sendgrid/README.md)             | SendGrid v3 API integration backend.                                                                                                                                                                                                                       |
|                         | [`email-sender-smtp`](crates/email-sender-smtp/README.md)                     | [`crates/email-sender-smtp`](crates/email-sender-smtp/README.md)                     | SMTP backend using `lettre` with STARTTLS & TLS support.                                                                                                                                                                                                   |
| **Event Sourcing**      | [`event-sourcing`](crates/event-sourcing/README.md)                           | [`crates/event-sourcing`](crates/event-sourcing/README.md)                           | Dynamic Consistency Boundaries (DCB) core specification ([dcb.events](https://dcb.events/specification/)), types (`SequencePosition`, `Query`, `AppendCondition`), and `EventStore` trait. Optional in-memory store behind `memory` feature.               |
|                         | [`event-store-postgres`](crates/event-store-postgres/README.md)               | [`crates/event-store-postgres`](crates/event-store-postgres/README.md)               | Production-grade PostgreSQL `EventStore` implementation using GIN array indexing and atomic `SERIALIZABLE` transactions.                                                                                                                                   |
|                         | [`event-store-umadb`](crates/event-store-umadb/README.md)                     | [`crates/event-store-umadb`](crates/event-store-umadb/README.md)                     | UmaDB gRPC `EventStore` implementation for Dynamic Consistency Boundaries ([umadb.io](https://umadb.io)).                                                                                                                                                  |
| **KV Store**            | [`kv-store`](crates/kv-store/README.md)                                       | [`crates/kv-store`](crates/kv-store/README.md)                                       | Core Key-Value specification, types, batch operations, range scanning, and `KvStore` trait. Optional in-memory store behind `memory` feature.                                                                                                              |
|                         | [`kv-store-postgres`](crates/kv-store-postgres/README.md)                     | [`crates/kv-store-postgres`](crates/kv-store-postgres/README.md)                     | PostgreSQL `KvStore` implementation using `sqlx` and atomic transactions.                                                                                                                                                                                  |
|                         | [`kv-store-redb`](crates/kv-store-redb/README.md)                             | [`crates/kv-store-redb`](crates/kv-store-redb/README.md)                             | Embedded persistent `KvStore` implementation backed by the `redb` ACID storage engine.                                                                                                                                                                     |
|                         | [`kv-store-redis`](crates/kv-store-redis/README.md)                           | [`crates/kv-store-redis`](crates/kv-store-redis/README.md)                           | Redis `KvStore` implementation using `redis-rs` async connection manager.                                                                                                                                                                                  |
| **Password Hasher**     | [`password-hasher`](crates/password-hasher/README.md)                         | [`crates/password-hasher`](crates/password-hasher/README.md)                         | Core `PasswordHasher` & `AsyncPasswordHasher` traits, PHC hash format auto-detection (`$argon2id$`, `$2b$`, `$noop$`), and error types. Optional `NoopHasher` behind `noop` feature.                                                                       |
|                         | [`password-hasher-argon2`](crates/password-hasher-argon2/README.md)           | [`crates/password-hasher-argon2`](crates/password-hasher-argon2/README.md)           | Argon2id memory-hard algorithm implementation with OWASP recommended default parameters and Tokio threadpool offloading.                                                                                                                                   |
|                         | [`password-hasher-bcrypt`](crates/password-hasher-bcrypt/README.md)           | [`crates/password-hasher-bcrypt`](crates/password-hasher-bcrypt/README.md)           | Bcrypt adaptive algorithm implementation with configurable cost factor and Tokio threadpool offloading.                                                                                                                                                    |
| **Password Policy**     | [`password-policy`](crates/password-policy/README.md)                         | [`crates/password-policy`](crates/password-policy/README.md)                         | Comprehensive password policy enforcement, OWASP/NIST 800-63B presets, context-aware user details check, bit entropy strength estimation, policy-compliant password generator, and optional HIBP breach check API integration.                             |
| **Secret Store**        | [`secret-store`](crates/secret-store/README.md)                               | [`crates/secret-store`](crates/secret-store/README.md)                               | Core Secret Store specification, Envelope Encryption (DEK + KEK), AEAD ciphers (`Aes256Gcm`, `ChaCha20Poly1305`), versioned `KeyRing`, and `SecretStore` trait. Optional in-memory store behind `memory` feature.                                          |
|                         | [`secret-store-postgres`](crates/secret-store-postgres/README.md)             | [`crates/secret-store-postgres`](crates/secret-store-postgres/README.md)             | PostgreSQL `SecretStore` implementation using `sqlx`, JSONB tag indexing & transactional DEK re-wrapping key rotation.                                                                                                                                     |
| **SMS Delivery**        | [`sms-sender`](crates/sms-sender/README.md)                                   | [`crates/sms-sender`](crates/sms-sender/README.md)                                   | Core traits, domain models (`SmsMessage`, `SmsError`), and `SmsSender` trait. Optional `MemorySmsSender` behind `memory` feature.                                                                                                                          |
| **Template Engine**     | [`template-engine`](crates/template-engine/README.md)                         | [`crates/template-engine`](crates/template-engine/README.md)                         | Generic template rendering specification, `TemplateContext` builder & `TemplateEngine` trait.                                                                                                                                                              |
|                         | [`template-engine-askama`](crates/template-engine-askama/README.md)           | [`crates/template-engine-askama`](crates/template-engine-askama/README.md)           | Askama (type-safe compile-time HTML templates) implementation of `TemplateEngine`.                                                                                                                                                                         |
|                         | [`template-engine-tera`](crates/template-engine-tera/README.md)               | [`crates/template-engine-tera`](crates/template-engine-tera/README.md)               | Tera (Jinja2-compatible) implementation of `TemplateEngine` with string, memory & recursive folder loading.                                                                                                                                                |
| **Two-Factor Auth**     | [`two-factor-auth`](crates/two-factor-auth/README.md)                         | [`crates/two-factor-auth`](crates/two-factor-auth/README.md)                         | Generic `TwoFactorProvider` trait, 2FA challenge/response models (`TwoFactorMethod::*`: TOTP, SMS, Email, BackupCode), and `BackupCode` helper. Optional `MemoryTwoFactorAuth` behind `memory` feature.                                                    |
|                         | [`two-factor-auth-totp`](crates/two-factor-auth-totp/README.md)               | [`crates/two-factor-auth-totp`](crates/two-factor-auth-totp/README.md)               | Production 2FA TOTP backend backed by `totp-rs` supporting `TotpTwoFactorAuth`, QR code rendering (base64 Data URIs & PNG), time skew tolerance, and generic `TwoFactorProvider`.                                                                          |
|                         | [`two-factor-auth-backup-code`](crates/two-factor-auth-backup-code/README.md) | [`crates/two-factor-auth-backup-code`](crates/two-factor-auth-backup-code/README.md) | Production 2FA Backup / Recovery Code backend supporting `BackupCodeTwoFactorAuth`, SHA-256 code hashing, single-use consumption, and generic `TwoFactorProvider`.                                                                                         |
|                         | [`two-factor-auth-sms`](crates/two-factor-auth-sms/README.md)                 | [`crates/two-factor-auth-sms`](crates/two-factor-auth-sms/README.md)                 | Production 2FA SMS OTP backend supporting `SmsTwoFactorAuth`, pluggable `SmsProvider` trait, `MemorySmsProvider`, TTL expiration via `SecretStore`, and generic `TwoFactorProvider`.                                                                       |
|                         | [`two-factor-auth-email`](crates/two-factor-auth-email/README.md)             | [`crates/two-factor-auth-email`](crates/two-factor-auth-email/README.md)             | Production 2FA Email OTP backend supporting `EmailTwoFactorAuth`, integration with `email-sender`, TTL expiration via `SecretStore`, and generic `TwoFactorProvider`.                                                                                      |
| **WebAuthn / Passkeys** | [`webauthn`](crates/webauthn/README.md)                                       | [`crates/webauthn`](crates/webauthn/README.md)                                       | Production WebAuthn / Passkeys primary passwordless authentication engine backed by `webauthn-rs`, `WebAuthnPolicy`, and `SecretStore`.                                                                                                                    |

---

## Getting Started

Add the required crates to your `Cargo.toml`. For unit testing, enable the `memory` feature on core crates to run without external database infrastructure:

```toml
[dependencies]
kv-store = { path = "crates/kv-store", features = ["memory"] }
password-hasher-argon2 = { path = "crates/password-hasher-argon2" }
secret-store = { path = "crates/secret-store", features = ["memory"] }
webauthn = { path = "crates/webauthn" }
```

### Quick Example: In-Memory Integration

```rust
use kv_store::memory::MemoryKvStore;
use kv_store::{KvStore, SetOptions};
use password_hasher_argon2::{Argon2Hasher, PasswordHasher};
use secret_store::memory::MemorySecretStore;
use secret_store::{CipherAlgorithm, KeyRing, MasterKey, SecretPath, SecretStore, SecretValue, KEY_LEN};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Password Hashing (Argon2id)
    let hasher = Argon2Hasher::new();
    let hash = hasher.hash_password("super_secret_password")?;
    assert!(hasher.verify_password("super_secret_password", &hash)?);

    // 2. Secret Store (Envelope Encryption)
    let master_key = MasterKey::new(1, [7u8; KEY_LEN]);
    let keyring = KeyRing::new([master_key])?;
    let secret_store = MemorySecretStore::new(keyring, CipherAlgorithm::Aes256Gcm);

    let path = SecretPath::new("prod/db/pass")?;
    secret_store.set(path.clone(), SecretValue::from("db_password_123"), Default::default()).await?;

    // 3. KV Store
    let kv = MemoryKvStore::new();
    kv.set(b"session:user_1", b"active_token", SetOptions::new()).await?;

    println!("All backend-kit components initialized successfully!");
    Ok(())
}
```

---

## Development & Makefile Commands

```bash
make help            # Show all available commands
make install-tools   # Install required cargo tools
make lint            # Run cargo clippy across workspace
make fmt             # Format all code (Rust, SQL, YAML, JSON, MD)
make sort            # Sort Cargo.toml dependencies
make remove-unused   # Remove unused dependencies
make upgrade         # Upgrade dependencies (compatible versions)
make upgrade-latest  # Upgrade dependencies to latest
make crate-add-lib xxx  # Create new library crate in workspace
make crate-remove xxx   # Remove crate from workspace
make version-patch   # Bump patch version (0.1.0 -> 0.1.1)
make version-minor   # Bump minor version (0.1.0 -> 0.2.0)
make version-major   # Bump major version (0.1.0 -> 1.0.0)
```

## Requirements

- **Rust**: 2024 edition (stable toolchain)
- **Tools**: Run `make install-tools` to install workspace tooling

---

## License

Licensed under MIT.
