# 🗄️ kv-store

Production-ready, high-performance Key-Value Store for Rust applications with PostgreSQL backend and conditional operations.

---

## ✨ Features

- 🔑 **Key-Value Persistence**: Store and query arbitrary JSON payloads indexed by string keys.
- 🔄 **Atomic Conditional Updates**: Support for create-if-absent and compare-and-swap (CAS) via [`PutCondition`].
- 🐘 **PostgreSQL Engine**: Built-in [`PostgresKvStore`] leveraging PostgreSQL advisory transaction locks for per-key serialization.
- 📊 **OpenTelemetry Built-in**: Out-of-the-box metrics (`db_kv_store_puts_total`, `db_kv_store_put_duration_seconds`, `db_kv_store_gets_total`, `db_kv_store_deletes_total`).
- ⏰ **Automatic Timestamps**: Accurate UTC `created_at` and `updated_at` timestamps maintained automatically.
- 🧪 **Mocking Ready**: Ships with [`MockKvStore`] generated via `mockall` for seamless unit testing.

---

## 🚀 Quick Start

### 1. Add Dependency

Add `kv-store` to your `Cargo.toml`:

```toml
[dependencies]
kv-store = "0.1"
```

### 2. Usage Example

```rust
use kv_store::{KV_STORE_MIGRATOR, KvStore, PostgresKvStore, PutCondition};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Connect to PostgreSQL
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://postgres:password@localhost:5432/kv_store_db")
        .await?;

    // 2. Run embedded migrations automatically
    KV_STORE_MIGRATOR.run(&pool).await?;

    // 3. Initialize Key-Value Store
    let store = PostgresKvStore::new(pool);

    // 4. Store a value conditionally (Create-if-absent)
    let user_profile = json!({
        "username": "alice",
        "email": "alice@example.com",
        "roles": ["admin"]
    });
    store
        .put("user:alice", user_profile, Some(PutCondition::Absent))
        .await?;

    // 5. Retrieve Stored Value
    if let Some(stored) = store.get("user:alice").await? {
        println!("Loaded value for key '{}': {:?}", stored.key, stored.value);
        println!("Created at: {:?}", stored.created_at);
        println!("Updated at: {:?}", stored.updated_at);
    }

    // 6. Compare-and-Swap (CAS update)
    let updated_profile = json!({
        "username": "alice",
        "email": "alice_updated@example.com",
        "roles": ["admin", "superadmin"]
    });
    let previous = json!({
        "username": "alice",
        "email": "alice@example.com",
        "roles": ["admin"]
    });
    store
        .put(
            "user:alice",
            updated_profile,
            Some(PutCondition::Value(previous)),
        )
        .await?;

    // 7. Delete Key
    if let Some(deleted) = store.delete("user:alice").await? {
        println!("Deleted key '{}' with value {:?}", deleted.key, deleted.value);
    }

    Ok(())
}
```

---

## 🧪 Unit Testing with Mock

[`MockKvStore`] allows testing domain logic without spinning up a real database:

```rust
use kv_store::{KvStore, MockKvStore, PutCondition, StoredValue};
use serde_json::json;

#[tokio::test]
async fn test_service_with_mock_store() {
    let mut mock = MockKvStore::new();
    mock.expect_get()
        .with(mockall::predicate::eq("config:features"))
        .returning(|key| {
            Ok(Some(StoredValue {
                key: key.to_string(),
                value: json!({ "dark_mode": true }),
                created_at: None,
                updated_at: None,
            }))
        });

    let res = mock.get("config:features").await.unwrap();
    assert!(res.is_some());
}
```

---

## 🗄️ Database Schema

The PostgreSQL table schema includes descriptions on all columns:

| Column | Type | Description |
| :--- | :--- | :--- |
| 🔑 `key` | `TEXT` | Primary Key. Unique string identifier for the key-value entry. |
| 📦 `value` | `JSONB` | Structured JSON payload stored under the key. |
| ⏰ `created_at` | `TIMESTAMPTZ` | Timestamp when the key-value entry was created. |
| 🔄 `updated_at` | `TIMESTAMPTZ` | Timestamp when the entry was last updated. |

---

## 📜 License

Licensed under the [MIT License](LICENSE).
