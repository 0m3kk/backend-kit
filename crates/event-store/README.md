# ⚡ event-store

High-performance, production-ready, [DCB (Dynamic Consistency Boundary)](https://dcb.events/) compliant Event Store for Rust applications.

---

## ✨ Features

- 🎯 **DCB Compliant**: Strict adherence to Dynamic Consistency Boundary specification for event-driven systems.
- 🐘 **PostgreSQL Engine**: Built-in [`PostgresEventStore`] with optimistic concurrency control, serializable isolation support, and batch operations.
- ⚡ **Stream Powered**: Async streaming querying powered by `tokio` and `async-stream`.
- 📊 **OpenTelemetry Built-in**: Out-of-the-box metrics (`appends_total`, `append_duration_seconds`, `read_duration_seconds`, etc.).
- 🔔 **Append Notifications**: Ships a migration that emits `NOTIFY events_appended` after each insert, so projection workers can wake immediately instead of polling.
- 🧪 **Mocking Ready**: Ships with [`MockEventStore`] generated via `mockall` for seamless unit testing.

---

## 🚀 Quick Start

### 1. Add Dependency

Add `event-store` to your `Cargo.toml`:

```toml
[dependencies]
event-store = "0.1"
```

### 2. Usage Example

```rust
use event_store::{Event, EventStore, PostgresEventStore, Query, EVENT_STORE_MIGRATOR};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Connect to PostgreSQL
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://postgres:password@localhost:5432/event_store_db")
        .await?;

    // 2. Run embedded migrations automatically
    EVENT_STORE_MIGRATOR.run(&pool).await?;

    // 3. Initialize Event Store
    let store = PostgresEventStore::new(pool);

    // 4. Append Events
    let events = vec![Event::new(
        "order:created",
        serde_json::json!({
            "order_id": "ord_98765",
            "amount": 250.00,
            "currency": "USD"
        }),
        vec!["order:ord_98765".into(), "user:usr_123".into()],
    )];
    
    let sequenced = store.append(events, None).await?;
    println!("Appended event at position: {}", sequenced[0].position);

    // 5. Read Events Stream
    let mut stream = store.read(Query::all(), None).await?;
    while let Some(event_result) = futures_util::StreamExt::next(&mut stream).await {
        let event = event_result?;
        println!("[{}] Type: {} Data: {}", event.position, event.event.event_type, event.event.data);
    }

    Ok(())
}
```

---

## 🔔 Notifications

The embedded migrations install an `AFTER INSERT` trigger on `events` that runs
`pg_notify('events_appended', NEW.event_type)`. The channel name is exported as
[`EVENTS_NOTIFY_CHANNEL`].

Notifications are delivered after commit. They are hints, not a durability
mechanism: listeners should always catch up from their checkpoint and keep a
polling fallback for lost notifications.

---

## 🧪 Unit Testing with Mock

[`MockEventStore`] allows testing domain logic without spinning up a real database:

```rust
use event_store::{EventStore, MockEventStore};

#[tokio::test]
async fn test_domain_logic() {
    let mut mock = MockEventStore::new();
    mock.expect_append()
        .times(1)
        .returning(|_, _| Ok(vec![]));

    // Inject `mock` into your service handlers
}
```

---

## 🗄️ Database Schema

The PostgreSQL table schema includes descriptions on all columns:

| Column | Type | Description |
| :--- | :--- | :--- |
| 🔑 `position` | `BIGINT` | Monotonically increasing sequence ID (Global event order). |
| 🏷️ `event_type` | `TEXT` | Domain event type identifier (e.g. `order_created`). |
| 📦 `data` | `JSONB` | Event payload in JSON format. |
| 📑 `metadata` | `JSONB` | Causation, correlation IDs, and actor context. |
| 🏷️ `tags` | `TEXT[]` | Stream/Aggregate identifiers indexed with GIN for fast queries. |

---

## 📜 License

Licensed under the [MIT License](LICENSE).
