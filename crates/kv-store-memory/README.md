# kv-store-memory

In-memory `KvStore` implementation for local development, testing, and high-performance caching.

## Features

- **Thread-Safe**: Powered by `Arc<RwLock<BTreeMap>>`.
- **TTL Support**: Passive/lazy expiration check on access.
- **Ordered Scanning**: Supports prefix, range, and reverse iteration.
- **Atomic Operations**: Atomic conditional mutations (`NX`/`XX`) and multi-key `batch` updates.
