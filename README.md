# 🧰 backend-kit

Composable building blocks for Rust backend services — HTTP, authentication, persistence, observability, and validation — developed together in a single Cargo workspace.

---

## 📦 Crates

| Crate | Description |
| --- | --- |
| 🧩 [`application`](crates/application) | Generic application primitives: execution context and event metadata helpers |
| 🔐 [`auth`](crates/auth) | JWT authentication middleware for axum services |
| 📄 [`config-loader`](crates/config-loader) | File and environment based configuration loading |
| ⚡ [`event-store`](crates/event-store) | DCB compliant event store: types, trait, and PostgreSQL implementation |
| 🌐 [`http-server`](crates/http-server) | HTTP router initialization, middleware, OpenAPI, and server configuration |
| 🎫 [`jwt`](crates/jwt) | Generic JSON Web Token (JWT) generation, verification, and configuration |
| 🗄️ [`kv-store`](crates/kv-store) | Key-value store: types, trait, and PostgreSQL implementation |
| 📧 [`mailer`](crates/mailer) | Email service abstraction, SMTP sender, and Minijinja template rendering |
| 🔑 [`oidc`](crates/oidc) | OpenID Connect (OIDC) authentication service, configuration, and helpers |
| 🔒 [`password`](crates/password) | Password policy validation and Argon2id hashing |
| 🐘 [`postgres-db`](crates/postgres-db) | PostgreSQL configuration, secrets, and connection management |
| 🔁 [`projection`](crates/projection) | Projection checkpoints, view contract, and catch-up worker |
| 🔐 [`secret-store`](crates/secret-store) | Secret store: types, trait, keyring, envelope encryption, and PostgreSQL implementation |
| 📸 [`snapshot-store`](crates/snapshot-store) | Snapshot store: types, trait, and PostgreSQL implementation |
| 📊 [`telemetry`](crates/telemetry) | OpenTelemetry tracing, metrics, OTLP export, and W3C context propagation |
| ✅ [`validation`](crates/validation) | Shared validation functions and helpers |
| 🛡️ [`webauthn`](crates/webauthn) | WebAuthn and Passkeys authentication service, configuration, and helpers |

Crates depend on each other by path: `application` and `projection` use
`event-store`, `auth` uses `http-server` and `jwt`, `postgres-db` uses
`telemetry`, and `snapshot-store` uses `event-store`.

---

## 🚀 Getting Started

### Prerequisites

- A Rust toolchain — pinned to `stable` via [`rust-toolchain.toml`](rust-toolchain.toml), with `rustfmt` and `clippy`.
- A PostgreSQL 18 server for the database-backed crates (`event-store`, `kv-store`, `secret-store`, `snapshot-store`, `postgres-db`).

### Build and test

Run any Cargo command from the repository root to operate on the whole workspace:

```sh
cargo build --workspace
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

To work on a single crate, pass `-p`:

```sh
cargo test -p jwt
```

### Database-backed tests

The store crates use `#[sqlx::test]`, which reads `DATABASE_URL` and provisions a throwaway database per test. Point it at a server where the configured user can create databases:

```sh
export DATABASE_URL=postgres://postgres:password@localhost:5432/dev
cargo test --workspace
```

The `postgres-db` tests default to `localhost:5432/dev` with `postgres` / `password`.

---

## 🧱 Workspace Notes

- **Edition 2024** with Cargo resolver `3`.
- Each crate keeps its own `README.md` and `LICENSE`.
- Clippy denies `unwrap_used` and `expect_used` across the workspace.

---

## 📜 License

Licensed under the MIT License. Each crate ships its own copy at `crates/<name>/LICENSE`.
