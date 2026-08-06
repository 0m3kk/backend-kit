# secret-store-memory

In-memory implementation of the `SecretStore` trait for backend-kit.

## Features

- **Concurrent & Thread-Safe**: Built on `tokio::sync::RwLock` for high-performance concurrent async operations.
- **Full Versioning & History**: Stores all historical secret versions per path and allows querying specific version numbers.
- **Encrypted Payload Storage**: Values are stored encrypted in memory using master keys managed by `KeyProvider`.
- **Master Key Rotation**: Re-encrypts secret payloads in-place when rotating master keys.
- **TTL Expiration & Purging**: Automatic expiration filtering and `clean_expired` purging logic.
