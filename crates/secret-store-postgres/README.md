# secret-store-postgres

PostgreSQL-backed implementation of the `SecretStore` trait for backend-kit.

## Features

- **Encrypted Relational Storage**: Stores encrypted secrets, AEAD ciphers (`Aes256Gcm`, `ChaCha20Poly1305`), nonces, and metadata in PostgreSQL via `sqlx`.
- **Atomic Version Management**: Monotonically increments version numbers per secret path transactionally.
- **Master Key Rotation**: Re-encrypts secret version records in PostgreSQL transactions when rotating master key IDs.
- **Fast Tag & Path Searches**: GIN indexing on JSONB tags and path prefix filtering.
- **TTL Support**: Automatic expiration logic and database cleanup routines.
