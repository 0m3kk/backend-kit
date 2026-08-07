# event-store-postgres

Production-grade PostgreSQL implementation of the [`EventStore`](../event-sourcing/README.md) trait from the `event-sourcing` crate, fully compliant with the [DCB specification](https://dcb.events/specification/).

## Features

- **Dynamic Consistency Boundaries**: Uses PostgreSQL GIN array index matching (`tags @> $1`) and `event_type = ANY($2)` for tag partitioning and conflict checking.
- **SERIALIZABLE Isolation & Retries**: Executes appends inside `SERIALIZABLE` transactions and automatically retries transient PostgreSQL errors (`40001` serialization failure and `40P01` deadlock) with exponential backoff.
- **Fast Conflict Check**: Uses `SELECT EXISTS(SELECT 1 FROM events ...)` to check `AppendCondition` without loading row payloads into memory.
- **Chunked Batch Insertion**: Multi-row batch `INSERT` statements chunked at `1,000` events per batch to prevent PostgreSQL's parameter limits.
- **Reactive Wire-Level Streaming**: Reactive `async_stream::stream!` returning owned `'static` streams.
- **Transactional API (`EventStoreTx<PgConnection>`)**: `append_tx` and `read_tx` methods accepting an external `&mut PgConnection`. The caller owns the transaction lifecycle (begin/commit/rollback). `read_tx` returns `Vec<SequencedEvent>` (streams cannot borrow from connections).

## Database Schema

```sql
CREATE TABLE IF NOT EXISTS events (
    id VARCHAR(255) NOT NULL,
    position BIGSERIAL PRIMARY KEY,
    event_type VARCHAR(255) NOT NULL,
    data JSONB NOT NULL,
    tags TEXT[] NOT NULL DEFAULT '{}',
    metadata JSONB,
    timestamp BIGINT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_events_id ON events (id);
CREATE INDEX IF NOT EXISTS idx_events_event_type_position ON events (event_type, position);
CREATE INDEX IF NOT EXISTS idx_events_tags ON events USING GIN (tags);
```

## Usage

Add `event-store-postgres` and `event-sourcing` to your `Cargo.toml`:

```toml
[dependencies]
event-sourcing = { path = "../event-sourcing" }
event-store-postgres = { path = "../event-store-postgres" }
sqlx = { version = "0.8", features = ["postgres", "runtime-tokio"] }
```

### Example

```rust
use event_sourcing::{Event, EventStore, Query, QueryItem, ReadOptions, Tag};
use event_store_postgres::PostgresEventStore;
use futures_util::StreamExt;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .connect("postgres://postgres:password@localhost:5432/my_db")
        .await?;

    let store = PostgresEventStore::new(pool);
    store.migrate().await?;

    let event = Event::new(
        "ev-101",
        "UserRegistered",
        serde_json::json!({"username": "alice"}),
        vec![Tag::key_value("username", "alice")],
    );

    let appended = store.append(vec![event], None).await?;
    println!("Appended at position: {}", appended[0].position);

    let query = Query::item(QueryItem::new().with_tag("username:alice"));
    let mut stream = store.read(&query, ReadOptions::new()).await;

    while let Some(res) = stream.next().await {
        let seq_event = res?;
        println!("Position {}: {:?}", seq_event.position, seq_event.event);
    }

    Ok(())
}
```
