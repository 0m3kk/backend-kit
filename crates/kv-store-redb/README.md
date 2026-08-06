# kv-store-redb

Embedded persistent `KvStore` implementation backed by the popular [`redb`](https://crates.io/crates/redb) ACID storage engine.

## Features

- **Embedded Disk Persistence**: Pure Rust embedded database operating directly on a single local file.
- **ACID Compliant**: Full transactional guarantees and crash safety.
- **TTL Support**: Millisecond-precision expiration timestamps encoded in payload storage.
- **Ordered Scanning**: B-tree range queries for prefix, range, and reverse scanning.
