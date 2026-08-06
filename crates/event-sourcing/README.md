# event-sourcing

Rust implementation of the **Dynamic Consistency Boundaries (DCB)** specification defined at [dcb.events/specification](https://dcb.events/specification/).

## Overview

DCB is an architectural pattern for event-sourced systems that decouples consistency boundaries from rigid aggregate streams. Instead of locking an entire stream, consistency boundaries are defined dynamically using query constraints over event types and domain tags.

## Specification & Trait Mapping

| DCB Specification Concept | Rust Type / Trait in `event-sourcing`  | Description                                                  |
| :------------------------ | :------------------------------------- | :----------------------------------------------------------- |
| **Domain Event Trait**    | [`DomainEvent`](src/types.rs#L5)       | Strongly-typed trait for domain event data payloads          |
| **Sequence Position**     | [`SequencePosition`](src/types.rs#L33) | Monotonic, unique event position (`u64`)                     |
| **Event Type**            | [`EventType`](src/types.rs#L65)        | String identifier of event type                              |
| **Tag**                   | [`Tag`](src/types.rs#L93)              | Domain partitioning tag (e.g. `user:123`, `account:acc-1`)   |
| **Event**                 | [`Event`](src/types.rs#L145)           | Unsequenced event with type, payload data, and tags          |
| **Sequenced Event**       | [`SequencedEvent`](src/types.rs#L185)  | Persisted event bound to a `SequencePosition`                |
| **Query Item**            | [`QueryItem`](src/types.rs#L210)       | Filter matching event types (OR) and tags (AND)              |
| **Query**                 | [`Query`](src/types.rs#L270)           | Combined query items with OR logic                           |
| **Append Condition**      | [`AppendCondition`](src/types.rs#L320) | Optimistic concurrency constraint for atomic append          |
| **Read Options**          | [`ReadOptions`](src/types.rs#L370)     | Filtering, direction, limit options                          |
| **Event Stream**          | [`EventStream`](src/store.rs#L27)      | Async reactive stream of `Result<SequencedEvent, ReadError>` |
| **Event Store Interface** | [`EventStore`](src/store.rs#L30)       | Core trait for async `read` (stream) and `append`            |

## Strongly-Typed Domain Event Usage

> Requires the `memory` feature: `event-sourcing = { version = "0.0.0", features = ["memory"] }`

```rust
use event_sourcing::{DomainEvent, EventStore, EventType, Tag};
use event_sourcing::memory::InMemoryEventStore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct OrderCreated {
    order_id: String,
    total_amount: f64,
}

impl DomainEvent for OrderCreated {
    fn event_type() -> EventType {
        EventType::new("OrderCreated")
    }

    fn tags(&self) -> Vec<Tag> {
        vec![Tag::key_value("order", &self.order_id)]
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = InMemoryEventStore::new();

    // 1. Create a strongly-typed domain event
    let domain_evt = OrderCreated {
        order_id: "ORD-123".to_string(),
        total_amount: 199.99,
    };

    // 2. Convert to unsequenced Event
    let event = domain_evt.to_event("evt-1")?;

    // 3. Append to Event Store
    let appended = store.append(vec![event], None).await?;

    // 4. Restore domain event from SequencedEvent
    let restored: OrderCreated = appended[0].to_domain_event()?;
    println!("Restored domain event: {:?}", restored);

    Ok(())
}
```
