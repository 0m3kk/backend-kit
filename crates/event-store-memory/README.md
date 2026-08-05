# event-store-memory

In-memory implementation of the [`EventStore`](../event-sourcing/README.md) trait from the `event-sourcing` crate, fully compliant with the [DCB specification](https://dcb.events/specification/).

## Usage

Add `event-store-memory` and `event-sourcing` to your `Cargo.toml`:

```toml
[dependencies]
event-sourcing = { path = "../event-sourcing" }
event-store-memory = { path = "../event-store-memory" }
```

### Example

```rust
use event_sourcing::{
    AppendCondition, Event, EventStore, Query, QueryItem, ReadOptions, Tag,
};
use event_store_memory::InMemoryEventStore;
use futures_util::StreamExt;

#[tokio::main]
async fn main() {
    let store = InMemoryEventStore::new();

    let event = Event::new(
        "ev-1",
        "UserRegistered",
        serde_json::json!({"username": "alice"}),
        vec![Tag::key_value("username", "alice")],
    );

    let appended = store.append(vec![event], None).await.unwrap();
    let pos = appended[0].position;

    let query = Query::item(QueryItem::new().with_tag("username:alice"));
    let mut stream = store.read(&query, ReadOptions::new()).await;

    while let Some(res) = stream.next().await {
        let seq_event = res.unwrap();
        println!("Pos {}: {:?}", seq_event.position, seq_event.event);
    }
}
```
