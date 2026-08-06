# cqrs

Command Query Responsibility Segregation (CQRS) crate for **backend-kit**, providing object-safe read-model views (`View<C>`), pluggable checkpoint persistence (`KvCheckpointStore`), on-demand catch-up query execution (`ViewQueryEngine`), and parallel multi-view background workers (`CatchupWorker`).

## Overview

In CQRS architectures, the Read Side is decoupled from Command processing. Domain events published by the Event Store are projected into specialized database read tables (Views).

This crate provides an unopinionated, storage-agnostic pipeline:

- **Storage Context Flexibility (`C`)**: `cqrs` does not dictate your database, ORM, or table schema. Views receive an arbitrary context `&C` (e.g., `&PgPool`, `&PgTransaction`, `&RedisClient`, or custom state).
- **Object-Safe Views (`View<C>`)**: Unifies view table naming, event filtering (`subscription_query`), and event projection logic (`apply_event`) into a single object-safe trait.
- **Parallel Catch-up Worker (`CatchupWorker`)**: Registers a list of views and catches them up to the latest events in parallel using `futures_util::future::join_all`.
- **Flexible Consistency (`ReadConsistency`)**: Choose between `Eventual` (immediate read) and `Strong` (synchronously catch up view to latest event before querying).

---

## Core Components & Trait Mapping

| CQRS Component            | Rust Type / Trait in `cqrs`                      | Description                                                                                    |
| :------------------------ | :----------------------------------------------- | :--------------------------------------------------------------------------------------------- |
| **Object-Safe View**      | [`View<C>`](src/view.rs)                         | Unified trait representing a read model and its event projection logic                         |
| **View Error**            | [`ViewError`](src/view.rs)                       | Error type for projection failure, storage errors, or deserialization errors                   |
| **Checkpoint Store**      | [`CheckpointStore`](src/view_checkpoint.rs)      | Abstract interface for retrieving and committing sequence positions per `view_name`            |
| **KV Checkpoint Adapter** | [`KvCheckpointStore<K>`](src/view_checkpoint.rs) | Adapter backing `CheckpointStore` using any `KvStore` (`kv-store-postgres`, Redis, etc.)       |
| **Read Consistency**      | [`ReadConsistency`](src/query.rs)                | Enum specifying consistency requirement (`Eventual`, `Strong`)                                 |
| **View Query Engine**     | [`ViewQueryEngine`](src/query.rs)                | Query executor providing on-demand catch-up to the latest Event Store head position            |
| **Multi-View Worker**     | [`CatchupWorker`](src/catchup_worker.rs)         | Concurrent worker managing a list of `Box<dyn View<C>>` views and catching them up in parallel |

---

## Defining a View (`View<C>`)

Implement `View<C>` on your read model or helper struct. Specify `view_name()`, optional `subscription_query()`, and `apply_event()`:

```rust
use async_trait::async_trait;
use cqrs::{View, ViewError};
use event_sourcing::{DomainEvent, EventType, Query, QueryItem, SequencedEvent};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserProfileView {
    pub user_id: String,
    pub username: String,
}

#[async_trait]
impl<C: MyDatabaseContext> View<C> for UserProfileView {
    fn view_name(&self) -> &'static str {
        "user_profiles"
    }

    /// Optional: filter events by type or tag. Defaults to Query::all().
    fn subscription_query(&self) -> Query {
        Query::item(QueryItem::new().with_type(UserRegistered::event_type()))
    }

    /// Project incoming domain event into storage context `C`.
    async fn apply_event(&self, event: &SequencedEvent, db: &C) -> Result<(), ViewError> {
        let payload: UserRegistered = serde_json::from_value(event.event.data.clone())
            .map_err(|e| ViewError::Deserialization(e.to_string()))?;

        // Execute SQL or storage mutation using `db` context
        db.insert_user(&payload.user_id, &payload.username).await?;
        Ok(())
    }
}
```

---

## Parallel Multi-View Catchup Worker (`CatchupWorker`)

Register multiple view instances in a `CatchupWorker` to process events in parallel across all views:

```rust
use cqrs::{CatchupWorker, KvCheckpointStore};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = MyDatabaseContext::new();
    let event_store = MyEventStore::new();
    let kv_store = MyKvStore::new();
    let checkpoint_store = KvCheckpointStore::new(kv_store);

    // Register multiple views
    let worker = CatchupWorker::new(db, event_store, checkpoint_store)
        .register_view(UserProfileView::default())
        .register_view(OrderSummaryView::default())
        .register_view(AnalyticsView::default());

    // Option A: Single parallel catch-up pass across all views
    let total_processed = worker.catchup_all().await?;
    println!("Processed {} events in parallel", total_processed);

    // Option B: Run continuously in a background polling loop
    tokio::spawn(worker.run_loop(Duration::from_secs(1)));

    Ok(())
}
```

---

## On-Demand Catch-up & Queries (`ViewQueryEngine`)

Execute queries with explicit consistency requirements:

- **`ReadConsistency::Eventual`**: Immediately executes read query against storage context `C` without waiting.
- **`ReadConsistency::Strong`**: Synchronously catches up projection to the latest event in `EventStore` before running query.

```rust
use cqrs::{ReadConsistency, ViewQueryEngine};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let query_engine = ViewQueryEngine::new(
        UserProfileView::default(),
        db_context,
        event_store,
        checkpoint_store,
    );

    // Strongly-consistent read (guarantees view is caught up to latest event before querying)
    let user = query_engine
        .query(ReadConsistency::Strong, |db| {
            db.find_user_by_id("user_100")
        })
        .await?;

    println!("Fetched latest user profile: {:?}", user);
    Ok(())
}
```

---

## Checkpoint Persistence (`KvCheckpointStore`)

Checkpoint tracking maps `view_name` -> `SequencePosition`. Wrap any `kv-store` backend (`kv-store-postgres`, `kv-store-redb`, `kv-store-redis`, or `MemoryKvStore`):

```rust
use cqrs::KvCheckpointStore;
use kv_store_postgres::PostgresKvStore;

let kv_store = PostgresKvStore::new(pool);
let checkpoint_store = KvCheckpointStore::with_prefix(kv_store, "cqrs:checkpoints:");
```

---

## License

Licensed under MIT.
