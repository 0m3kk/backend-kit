# event-sourcing

Rust implementation of the **Dynamic Consistency Boundaries (DCB)** specification defined at [dcb.events/specification](https://dcb.events/specification/).

## Overview

DCB is an architectural pattern for event-sourced systems that decouples consistency boundaries from rigid aggregate streams. Instead of locking an entire stream, consistency boundaries are defined dynamically using query constraints over event types and domain tags.

## Specification Mapping

| DCB Specification Concept | Rust Type / Trait in `event-sourcing`                   | Description                                                  |
| :------------------------ | :------------------------------------------------------ | :----------------------------------------------------------- |
| **Sequence Position**     | [`SequencePosition`](src/types.rs#L6)                   | Monotonic, unique event position (`u64`)                     |
| **Event Type**            | [`EventType`](src/types.rs#L34)                         | String identifier of event type                              |
| **Tag**                   | [`Tag`](src/types.rs#L58)                               | Domain partitioning tag (e.g. `user:123`, `account:acc-1`)   |
| **Event**                 | [`Event`](src/types.rs#L87)                             | Unsequenced event with type, payload data, and tags          |
| **Sequenced Event**       | [`SequencedEvent`](src/types.rs#L122)                   | Persisted event bound to a `SequencePosition`                |
| **Query Item**            | [`QueryItem`](src/types.rs#L149)                        | Filter matching event types (OR) and tags (AND)              |
| **Query**                 | [`Query`](src/types.rs#L210)                            | Combined query items with OR logic                           |
| **Append Condition**      | [`AppendCondition`](src/types.rs#L260)                  | Optimistic concurrency constraint for atomic append          |
| **Read Options**          | [`ReadOptions`](src/types.rs#L311)                      | Filtering, direction, limit options                          |
| **Event Stream**          | [`EventStream`](src/store.rs#L27)                       | Async reactive stream of `Result<SequencedEvent, ReadError>` |
| **Event Store Interface** | [`EventStore`](src/store.rs#L30)                        | Core trait for async `read` (stream) and `append`            |
| **In-Memory Store**       | [`InMemoryEventStore`](../event-store-memory/README.md) | Thread-safe reference implementation                         |

## Usage Example

```rust
use event_sourcing::{
    AppendCondition, Event, EventStore, InMemoryEventStore,
    Query, QueryItem, ReadOptions, SequencePosition, Tag,
};
use futures_util::StreamExt;

#[tokio::main]
async fn main() {
    let store = InMemoryEventStore::new();

    // 1. Create an event with id and domain tags
    let event = Event::new(
        "ev-1",
        "UserRegistered",
        serde_json::json!({"username": "alice"}),
        vec![Tag::key_value("username", "alice")],
    );

    // 2. Append event to store
    let appended = store.append(vec![event], None).await.unwrap();
    let pos = appended[0].position;

    // 3. Read matching events as an async stream
    let query = Query::item(QueryItem::new().with_tag("username:alice"));
    let mut stream = store.read(&query, ReadOptions::new()).await;

    while let Some(res) = stream.next().await {
        let seq_event = res.unwrap();
        println!("Event position {}: {:?}", seq_event.position, seq_event.event);
    }

    // 4. Append with consistency condition (fail if username:alice event exists after pos)
    let condition = AppendCondition::new(query).after(pos);
    let duplicate = Event::new(
        "UserRegistered",
        serde_json::json!({"username": "alice"}),
        vec![Tag::key_value("username", "alice")],
    );

    let result = store.append(vec![duplicate], Some(condition)).await;
    println!("Append result: {:?}", result);
}
```
