# secret-store-memory

In-memory implementation of the `SecretStore` trait for backend-kit.

## Features

- **Concurrent & Thread-Safe**: Built on `tokio::sync::RwLock` for high-performance concurrent async operations.
- **Envelope Encryption**: Values are stored encrypted with random DEKs wrapped under versioned KEKs from a `KeyRing`.
- **Full Versioning & History**: Stores all historical secret versions per path and allows querying specific version numbers.
- **Dynamic Key Rotation**: `rotate_key()` re-wraps stored DEKs under the current master key version without modifying or decrypting main secret payloads.
- **TTL Expiration & Purging**: Automatic expiration filtering and `clean_expired` purging logic.
