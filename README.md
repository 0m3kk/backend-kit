# Backend Kit

A collection of foundational Rust backend libraries and components.

## Workspace Crates Index

| Crate                       | Path                                                                     | Description                                                                                                                           |
| :-------------------------- | :----------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------ |
| **`event-sourcing`**        | [`crates/event-sourcing`](crates/event-sourcing/README.md)               | DCB (Dynamic Consistency Boundaries) core specification, types & `EventStore` trait ([dcb.events](https://dcb.events/specification/)) |
| **`event-store-memory`**    | [`crates/event-store-memory`](crates/event-store-memory/README.md)       | In-memory `EventStore` implementation for local development and testing                                                               |
| **`event-store-postgres`**  | [`crates/event-store-postgres`](crates/event-store-postgres/README.md)   | PostgreSQL `EventStore` implementation using GIN array indexing and atomic transactions                                               |
| **`event-store-umadb`**     | [`crates/event-store-umadb`](crates/event-store-umadb/README.md)         | UmaDB gRPC `EventStore` implementation for Dynamic Consistency Boundaries ([umadb.io](https://umadb.io))                              |
| **`kv-store`**              | [`crates/kv-store`](crates/kv-store/README.md)                           | Core Key-Value Store specification, types & `KvStore` trait                                                                           |
| **`kv-store-memory`**       | [`crates/kv-store-memory`](crates/kv-store-memory/README.md)             | In-memory concurrent `KvStore` implementation with TTL & range scanning                                                               |
| **`kv-store-postgres`**     | [`crates/kv-store-postgres`](crates/kv-store-postgres/README.md)         | PostgreSQL `KvStore` implementation using `sqlx` and atomic transactions                                                              |
| **`kv-store-redb`**         | [`crates/kv-store-redb`](crates/kv-store-redb/README.md)                 | Embedded persistent `KvStore` implementation backed by `redb` ACID storage engine                                                     |
| **`kv-store-redis`**        | [`crates/kv-store-redis`](crates/kv-store-redis/README.md)               | Redis `KvStore` implementation using `redis-rs` async connection manager                                                              |
| **`secret-store`**          | [`crates/secret-store`](crates/secret-store/README.md)                   | Core Secret Store specification, types, cryptography (AES-GCM / ChaCha20Poly1305) & `SecretStore` trait                               |
| **`secret-store-memory`**   | [`crates/secret-store-memory`](crates/secret-store-memory/README.md)     | In-memory concurrent `SecretStore` implementation with AEAD encryption, versioning, path hierarchy & key rotation                     |
| **`secret-store-postgres`** | [`crates/secret-store-postgres`](crates/secret-store-postgres/README.md) | PostgreSQL `SecretStore` implementation using `sqlx`, JSONB tag indexing & transactional key rotation                                 |

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
