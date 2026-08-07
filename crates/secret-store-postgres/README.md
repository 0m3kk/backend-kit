# secret-store-postgres

PostgreSQL-backed implementation of the `SecretStore` trait for backend-kit.

## Features

- **Envelope Encryption**: Stores encrypted secrets, AEAD ciphers (`Aes256Gcm`, `ChaCha20Poly1305`), DEKs wrapped by `KeyRing` KEKs, nonces, and metadata in PostgreSQL via `sqlx`.
- **Atomic Version Management**: Monotonically increments version numbers per secret path transactionally.
- **Transactional Key Rotation**: `rotate_key()` re-wraps DEKs in PostgreSQL transactions when upgrading to new master key versions.
- **Fast Tag & Path Searches**: GIN indexing on JSONB tags and path prefix filtering on `secret_headers`.
- **TTL Support**: Automatic expiration logic and database cleanup routines.
- **Transactional API (`SecretStoreTx<PgConnection>`)**: All operations available as `_tx` methods accepting an external `&mut PgConnection`. The caller owns the transaction lifecycle (begin/commit/rollback).

## Usage

### Standalone (auto-acquires connection)

```rust
use secret_store::{SecretPath, SecretStore, SecretValue, SetSecretOptions};
use secret_store_postgres::PostgresSecretStore;

let store = PostgresSecretStore::new(pool, keyring, cipher);
store.migrate().await?;

let path = SecretPath::new("prod/db/password")?;
store.set(path.clone(), SecretValue::from("secret"), SetSecretOptions::new()).await?;
let entry = store.get(&path).await?;
```

### Within an external transaction

```rust
use secret_store::{SecretPath, SecretStoreTx, SecretValue, SetSecretOptions};
use secret_store_postgres::PostgresSecretStore;

let store = PostgresSecretStore::new(pool, keyring, cipher);
let mut tx = pool.begin().await?;

let path = SecretPath::new("prod/db/password")?;
<SecretStoreTx<sqlx::PgConnection>>::set_tx(&store, &mut tx, path.clone(), SecretValue::from("secret"), SetSecretOptions::new()).await?;
let entry = <SecretStoreTx<sqlx::PgConnection>>::get_tx(&store, &mut tx, &path).await?;

tx.commit().await?;
```
