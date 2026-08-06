# secret-store-postgres

PostgreSQL-backed implementation of the `SecretStore` trait for backend-kit.

## Features

- **Envelope Encryption**: Stores encrypted secrets, AEAD ciphers (`Aes256Gcm`, `ChaCha20Poly1305`), DEKs wrapped by `KeyRing` KEKs, nonces, and metadata in PostgreSQL via `sqlx`.
- **Atomic Version Management**: Monotonically increments version numbers per secret path transactionally.
- **Transactional Key Rotation**: `rotate_key()` re-wraps DEKs in PostgreSQL transactions when upgrading to new master key versions.
- **Fast Tag & Path Searches**: GIN indexing on JSONB tags and path prefix filtering on `secret_headers`.
- **TTL Support**: Automatic expiration logic and database cleanup routines.
