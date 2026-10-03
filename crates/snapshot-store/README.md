# 📸 snapshot-store

[DCB](https://dcb.events/) compliant **Snapshot Store** contract and PostgreSQL implementation for Rust applications.

A snapshot is the persisted, folded state of a domain model up to a sequence position, used to optimize aggregate loading in event-sourced systems.

---

## ✨ Features

- 📐 **DCB Compliant**: Implements the [Snapshot Store specification](https://dcb-events.github.io/docs/specification/) for Dynamic Consistency Boundary architectures.
- 🐘 **PostgreSQL Engine**: High-performance persistence backed by PostgreSQL JSONB with [`PostgresSnapshotStore`].
- ⚡ **Threshold Filtering**: Configurable event threshold (`with_threshold`) to avoid unnecessary snapshots for low-event aggregates.
- 🔄 **Upsert Semantics**: Seamless atomic upsert operations (`ON CONFLICT (key) DO UPDATE`) keeping the latest folded state and sequence position.
- 🧪 **Mocking Ready**: Ships with [`MockSnapshotStore`] generated via `mockall` for fast, dependency-free unit testing.

---

## 🚀 Quick Start

### 1. Add Dependency

Add `snapshot-store` to your `Cargo.toml`:

```toml
[dependencies]
snapshot-store = "0.1"
```

### 2. Usage Example

```rust
use serde_json::json;
use snapshot_store::{
    PostgresSnapshotStore, Snapshot, SnapshotStore, SNAPSHOT_STORE_MIGRATOR,
};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Connect to PostgreSQL
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://postgres:password@localhost:5432/snapshot_store_db")
        .await?;

    // 2. Run embedded database migrations automatically
    SNAPSHOT_STORE_MIGRATOR.run(&pool).await?;

    // 3. Initialize store with optional replayed events threshold (e.g. 100)
    let store = PostgresSnapshotStore::new(pool).with_threshold(100);

    // 4. Check threshold during aggregate/model loading
    let replayed_events = 150;
    if store.should_snapshot(replayed_events) {
        let snapshot = Snapshot::new(Some(150), json!({"balance": 500}));
        store.put("account:1", snapshot).await?;
        println!("Snapshot saved successfully");
    }

    // 5. Read snapshot
    if let Some(loaded) = store.get("account:1").await? {
        println!("Loaded snapshot at position {:?}: {:?}", loaded.position, loaded.state);
    }

    Ok(())
}
```

---

## 🧪 Unit Testing with Mock

[`MockSnapshotStore`] allows testing domain logic without spinning up a real database:

```rust
use serde_json::json;
use snapshot_store::{MockSnapshotStore, Snapshot, SnapshotStore};

#[tokio::test]
async fn test_service_with_mock_store() {
    let mut mock = MockSnapshotStore::new();
    mock.expect_get()
        .with(mockall::predicate::eq("account:1"))
        .returning(|_| Ok(Some(Snapshot::new(Some(42), json!({"balance": 100})))));

    // Inject `mock` into your aggregate loader or command handler
    let snapshot = mock.get("account:1").await.unwrap();
    assert!(snapshot.is_some());
}
```

---

## 🗄️ Database Schema

The PostgreSQL table schema includes descriptions on all columns:

| Column | Type | Description |
| :--- | :--- | :--- |
| 🔑 `key` | `TEXT` | Primary Key. Unique string identifier for the model (e.g. `order:123`, `account:1`). |
| 🔢 `position` | `BIGINT` | Position of the last event folded into the snapshot; `NULL` if none. |
| 📦 `state` | `JSONB` | Serialized model state stored as JSONB. |
| ⏰ `created_at` | `TIMESTAMPTZ` | Snapshot creation timestamp. |
| 🔄 `updated_at` | `TIMESTAMPTZ` | Snapshot last update timestamp. |

---

## 📜 License

Licensed under the [MIT License](LICENSE).
