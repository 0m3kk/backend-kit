# kv-store

Core specification, types, and trait definitions for Key-Value Stores in `backend-kit`.

## Overview

`kv-store` provides a unified, async Key-Value store interface (`KvStore`) that supports get, set, delete, atomic batch mutations, range scanning, and TTL expiration across multiple backends.

## Specification & Trait Mapping

| Concept            | Type / Trait in `kv-store`    | Description                                                       |
| :----------------- | :---------------------------- | :---------------------------------------------------------------- |
| **Key**            | [`Key`](src/types.rs)         | Binary/string key wrapper (`Vec<u8>`)                             |
| **Value**          | [`Value`](src/types.rs)       | Binary payload wrapper with serde JSON helpers                    |
| **KV Entry**       | [`KvEntry`](src/types.rs)     | Key-Value pair with optional `expires_at` timestamp               |
| **Set Options**    | [`SetOptions`](src/types.rs)  | Mutation options (`ttl`, `if_not_exists` [NX], `if_exists` [XX])  |
| **Batch Op**       | [`BatchOp`](src/types.rs)     | Atomic mutations (`Put`, `Delete`)                                |
| **Scan Options**   | [`ScanOptions`](src/types.rs) | Range query bounds (`prefix`, `start`, `end`, `limit`, `reverse`) |
| **KV Stream**      | [`KvStream`](src/store.rs)    | Owned async stream of `Result<KvEntry, KvError>`                  |
| **KV Store Trait** | [`KvStore`](src/store.rs)     | Unified interface implemented by all backends                     |
