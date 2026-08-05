# event-store-umadb

UmaDB implementation of the `EventStore` trait for [Dynamic Consistency Boundaries (DCB)](https://umadb.io).

## Features

- **UmaDB gRPC Integration**: Implements the [`EventStore`] trait backed by `umadb-client` and `umadb-dcb`.
- **Dynamic Consistency Boundaries**: Native support for DCB event querying and conflict detection.
- **gRPC Transport**: Supports secure gRPC connections with optional API key authentication.

## Usage

```rust
use event_sourcing::{Event, EventStore, Query, ReadOptions};
use event_store_umadb::UmaDBEventStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = UmaDBEventStore::connect("http://localhost:50051", None).await?;

    let event = Event::new("ev-1", "UserRegistered", serde_json::json!({"user_id": 42}), vec![]);
    store.append(vec![event], None).await?;

    let mut stream = store.read(&Query::all(), ReadOptions::new()).await;
    while let Some(res) = stream.next().await {
        let seq_evt = res?;
        println!("Received event position {}: {}", seq_evt.position, seq_evt.event.id);
    }

    Ok(())
}
```
