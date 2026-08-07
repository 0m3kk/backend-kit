# kv-store-postgres

PostgreSQL `KvStore` implementation using [`sqlx`](https://crates.io/crates/sqlx).

## Features

- **PostgreSQL Backed**: Stores key-value entries in a dedicated PostgreSQL table (`kv_entries`).
- **Atomic Transactions**: Multi-op batch updates using SQL transactions.
- **TTL Support**: `TIMESTAMPTZ` bounds with indexed expiration filters.
- **Auto-Migration**: Built-in `migrate()` method to set up schema and indexes.
- **Transactional API (`KvStoreTx<PgConnection>`)**: All operations available as `_tx` methods accepting an external `&mut PgConnection`. The caller owns the transaction lifecycle (begin/commit/rollback).

## Usage

### Standalone (auto-acquires connection)

```rust
use kv_store::{Key, KvStore, SetOptions, Value};
use kv_store_postgres::PostgresKvStore;

let store = PostgresKvStore::new(pool);
store.migrate().await?;

store.set(Key::from("k1"), Value::from("v1"), SetOptions::new()).await?;
let val = store.get(&Key::from("k1")).await?;
```

### Within an external transaction

```rust
use kv_store::{Key, KvStoreTx, SetOptions, Value};
use kv_store_postgres::PostgresKvStore;

let store = PostgresKvStore::new(pool);
let mut tx = pool.begin().await?;

// All operations participate in the same transaction
<KvStoreTx<sqlx::PgConnection>>::set_tx(&store, &mut tx, Key::from("k1"), Value::from("v1"), SetOptions::new()).await?;
let val = <KvStoreTx<sqlx::PgConnection>>::get_tx(&store, &mut tx, &Key::from("k1")).await?;

tx.commit().await?;
```
