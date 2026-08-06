# Backend Kit

A collection of foundational Rust backend libraries and components.

## Workspace Crates Index

| Crate                        | Path                                                                       | Description                                                                                                                           |
| :--------------------------- | :------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------ |
| **`email-sender`**           | [`crates/email-sender`](crates/email-sender/README.md)                     | Core traits, domain models, error definitions & fluent email builder (`EmailBuilder`)                                                 |
| **`email-sender-memory`**    | [`crates/email-sender-memory`](crates/email-sender-memory/README.md)       | In-memory mock `EmailSender` implementation for unit testing and local development                                                    |
| **`email-sender-resend`**    | [`crates/email-sender-resend`](crates/email-sender-resend/README.md)       | Resend API provider implementation wrapping the `resend-rs` SDK crate                                                                 |
| **`email-sender-sendgrid`**  | [`crates/email-sender-sendgrid`](crates/email-sender-sendgrid/README.md)   | SendGrid v3 API provider implementation wrapping the `sendgrid` SDK crate                                                             |
| **`email-sender-smtp`**      | [`crates/email-sender-smtp`](crates/email-sender-smtp/README.md)           | SMTP `EmailSender` implementation using `lettre` with STARTTLS & TLS support                                                          |
| **`event-sourcing`**         | [`crates/event-sourcing`](crates/event-sourcing/README.md)                 | DCB (Dynamic Consistency Boundaries) core specification, types & `EventStore` trait ([dcb.events](https://dcb.events/specification/)) |
| **`event-store-memory`**     | [`crates/event-store-memory`](crates/event-store-memory/README.md)         | In-memory `EventStore` implementation for local development and testing                                                               |
| **`event-store-postgres`**   | [`crates/event-store-postgres`](crates/event-store-postgres/README.md)     | PostgreSQL `EventStore` implementation using GIN array indexing and atomic transactions                                               |
| **`event-store-umadb`**      | [`crates/event-store-umadb`](crates/event-store-umadb/README.md)           | UmaDB gRPC `EventStore` implementation for Dynamic Consistency Boundaries ([umadb.io](https://umadb.io))                              |
| **`kv-store`**               | [`crates/kv-store`](crates/kv-store/README.md)                             | Core Key-Value Store specification, types & `KvStore` trait                                                                           |
| **`kv-store-memory`**        | [`crates/kv-store-memory`](crates/kv-store-memory/README.md)               | In-memory concurrent `KvStore` implementation with TTL & range scanning                                                               |
| **`kv-store-postgres`**      | [`crates/kv-store-postgres`](crates/kv-store-postgres/README.md)           | PostgreSQL `KvStore` implementation using `sqlx` and atomic transactions                                                              |
| **`kv-store-redb`**          | [`crates/kv-store-redb`](crates/kv-store-redb/README.md)                   | Embedded persistent `KvStore` implementation backed by `redb` ACID storage engine                                                     |
| **`kv-store-redis`**         | [`crates/kv-store-redis`](crates/kv-store-redis/README.md)                 | Redis `KvStore` implementation using `redis-rs` async connection manager                                                              |
| **`password-hasher`**        | [`crates/password-hasher`](crates/password-hasher/README.md)               | Universal password hashing library supporting Argon2id, Bcrypt, PBKDF2, PHC hash format auto-detection & async execution              |
| **`secret-store`**           | [`crates/secret-store`](crates/secret-store/README.md)                     | Core Secret Store specification, Envelope Encryption (DEK + KEK), `KeyRing` & `SecretStore` trait                                     |
| **`secret-store-memory`**    | [`crates/secret-store-memory`](crates/secret-store-memory/README.md)       | In-memory concurrent `SecretStore` implementation with Envelope Encryption, versioning, path hierarchy & `KeyRing` rotation           |
| **`secret-store-postgres`**  | [`crates/secret-store-postgres`](crates/secret-store-postgres/README.md)   | PostgreSQL `SecretStore` implementation using `sqlx`, JSONB tag indexing & transactional DEK re-wrapping key rotation                 |
| **`template-engine`**        | [`crates/template-engine`](crates/template-engine/README.md)               | Generic template rendering specification, `TemplateContext` builder & `TemplateEngine` trait                                          |
| **`template-engine-askama`** | [`crates/template-engine-askama`](crates/template-engine-askama/README.md) | Askama (type-safe compile-time HTML templates) implementation of `TemplateEngine`                                                     |
| **`template-engine-tera`**   | [`crates/template-engine-tera`](crates/template-engine-tera/README.md)     | Tera (Jinja2-compatible) implementation of `TemplateEngine` with string, memory & recursive folder template loading                   |

---

## Commands

```bash
make help            # Show all commands
make install-tools   # Install required cargo tools
make lint            # Run clippy
make fmt             # Format all code (Rust, SQL, YAML, JSON, MD)
make sort            # Sort Cargo.toml dependencies
make remove-unused   # Remove unused dependencies
make upgrade         # Upgrade dependencies (compatible versions)
make upgrade-latest  # Upgrade dependencies to latest
make sql-fmt         # Format SQL files
make prettier        # Format YAML, JSON, MD files
make crate-add-lib xxx  # Add library crate
make crate-add-bin xxx  # Add binary crate
make crate-remove xxx   # Remove crate
make version-patch   # Bump patch version (0.1.0 -> 0.1.1)
make version-minor   # Bump minor version (0.1.0 -> 0.2.0)
make version-major   # Bump major version (0.1.0 -> 1.0.0)
```

## Requirements

- Rust (stable)
- Run `make install-tools` to install all dependencies
