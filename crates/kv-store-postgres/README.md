# kv-store-postgres

PostgreSQL `KvStore` implementation using [`sqlx`](https://crates.io/crates/sqlx).

## Features

- **PostgreSQL Backed**: Stores key-value entries in a dedicated PostgreSQL table (`kv_entries`).
- **Atomic Transactions**: Multi-op batch updates using SQL transactions.
- **TTL Support**: `TIMESTAMPTZ` bounds with indexed expiration filters.
- **Auto-Migration**: Built-in `migrate()` method to set up schema and indexes.
