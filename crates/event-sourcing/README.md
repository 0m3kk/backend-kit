# event-sourcing

Rust implementation of the **Dynamic Consistency Boundaries (DCB)** specification defined at [dcb.events/specification](https://dcb.events/specification/).

## Overview

DCB is an architectural pattern for event-sourced systems that decouples consistency boundaries from rigid aggregate streams. Instead of locking an entire stream, consistency boundaries are defined dynamically using query constraints over event types and domain tags.

## Specification & Trait Mapping

| DCB Specification Concept | Rust Type / Trait in `event-sourcing`      | Description                                                  |
| :------------------------ | :----------------------------------------- | :----------------------------------------------------------- |
| **Domain Event Trait**    | [`DomainEvent`](src/types.rs#L5)           | Strongly-typed trait for domain event data payloads          |
| **Sequence Position**     | [`SequencePosition`](src/types.rs#L33)     | Monotonic, unique event position (`u64`)                     |
| **Event Type**            | [`EventType`](src/types.rs#L65)            | String identifier of event type                              |
| **Tag**                   | [`Tag`](src/types.rs#L93)                  | Domain partitioning tag (e.g. `user:123`, `account:acc-1`)   |
| **Event**                 | [`Event`](src/types.rs#L145)               | Unsequenced event with type, payload data, and tags          |
| **Sequenced Event**       | [`SequencedEvent`](src/types.rs#L185)      | Persisted event bound to a `SequencePosition`                |
| **Query Item**            | [`QueryItem`](src/types.rs#L210)           | Filter matching event types (OR) and tags (AND)              |
| **Query**                 | [`Query`](src/types.rs#L270)               | Combined query items with OR logic                           |
| **Append Condition**      | [`AppendCondition`](src/types.rs#L320)     | Optimistic concurrency constraint for atomic append          |
| **Read Options**          | [`ReadOptions`](src/types.rs#L370)         | Filtering, direction, limit options                          |
| **Event Stream**          | [`EventStream`](src/store.rs#L27)          | Async reactive stream of `Result<SequencedEvent, ReadError>` |
| **Event Store Interface** | [`EventStore`](src/store.rs#L30)           | Core trait for async `read` (stream) and `append`            |
| **Decision Model**        | [`DecisionModel`](src/decision.rs)         | Trait for state projection and dynamic query specification   |
| **Loaded Model**          | [`LoadedModel`](src/decision.rs)           | Container tracking model state `M` and `last_position`       |
| **Event Store Extension** | [`EventStoreExt`](src/decision.rs)         | Extension trait for `load_decision_model`                    |
| **Snapshot Extension**    | [`EventStoreSnapshotExt`](src/snapshot.rs) | Extension trait for snapshot loading with catch-up           |
| **Snapshot Options**      | [`SnapshotOptions`](src/snapshot.rs)       | Configurable auto-save threshold options                     |

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

## Decision Model Usage

```rust
use event_sourcing::{
    DecisionModel, Event, EventStore, EventStoreExt, Query, QueryItem, Tag,
};
use event_sourcing::memory::InMemoryEventStore;

pub struct BankAccount {
    pub account_id: String,
    pub balance: i64,
}

impl BankAccount {
    pub fn new(account_id: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            balance: 0,
        }
    }
}

impl DecisionModel for BankAccount {
    fn query(&self) -> Query {
        Query::item(
            QueryItem::new()
                .with_types(["MoneyDeposited", "MoneyWithdrawn"])
                .with_tag(Tag::key_value("account", &self.account_id)),
        )
    }

    fn apply_event(&mut self, event: &Event) {
        match event.event_type.as_str() {
            "MoneyDeposited" => {
                let amount: i64 = serde_json::from_value(event.data["amount"].clone()).unwrap_or(0);
                self.balance += amount;
            }
            "MoneyWithdrawn" => {
                let amount: i64 = serde_json::from_value(event.data["amount"].clone()).unwrap_or(0);
                self.balance -= amount;
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = InMemoryEventStore::new();

    // Hydrate model directly from the store
    let account = store.load_decision_model(BankAccount::new("ACC-123")).await?;

    println!("Current balance: {}", account.balance);
    println!("Last sequence position: {:?}", account.last_position);

    Ok(())
}
```

## Decision Model Snapshot Usage

```rust
use event_sourcing::{
    DecisionModel, Event, EventStore, EventStoreSnapshotExt, Query, QueryItem, SnapshotOptions, Tag,
};
use event_sourcing::memory::InMemoryEventStore;
use kv_store::memory::MemoryKvStore;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct BankAccount {
    pub account_id: String,
    pub balance: i64,
}

impl BankAccount {
    pub fn new(account_id: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            balance: 0,
        }
    }
}

impl DecisionModel for BankAccount {
    fn query(&self) -> Query {
        Query::item(
            QueryItem::new()
                .with_types(["MoneyDeposited", "MoneyWithdrawn"])
                .with_tag(Tag::key_value("account", &self.account_id)),
        )
    }

    fn apply_event(&mut self, event: &Event) {
        match event.event_type.as_str() {
            "MoneyDeposited" => {
                let amount: i64 = serde_json::from_value(event.data["amount"].clone()).unwrap_or(0);
                self.balance += amount;
            }
            "MoneyWithdrawn" => {
                let amount: i64 = serde_json::from_value(event.data["amount"].clone()).unwrap_or(0);
                self.balance -= amount;
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = InMemoryEventStore::new();
    let kv_store = MemoryKvStore::new();

    // Hydrate using snapshot with auto-save threshold (e.g., auto-save every 50 events)
    let mut account = store
        .load_decision_model_with_snapshot(
            &kv_store,
            BankAccount::new("ACC-123"),
            SnapshotOptions::new(50),
        )
        .await?;

    // Manually persist snapshot to KV store if desired
    account.save_snapshot(&kv_store).await?;

    Ok(())
}
```
