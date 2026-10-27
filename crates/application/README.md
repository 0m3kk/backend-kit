# application

Generic application-layer primitives shared by backend services.

Currently provides:

- [`ExecutionContext`](src/context.rs), which carries request metadata (actor,
  correlation, tracing, client details) and converts domain events into
  `event-store` events enriched with that metadata.
- [`State`](src/state.rs) / [`StateLoader`](src/state.rs), which rehydrate a
  decision state from a snapshot and replay newer events.

## State loading

A state is a wrapper around a generated contract. It owns its snapshot key,
event query, guards, and reducer:

```rust
use application::State;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct ProductExistenceState(ProductExistenceStateContract);

impl State for ProductExistenceState {
    type Error = CatalogError;

    fn key(&self) -> String {
        format!("catalog.product_existence:{}", self.0.product_id)
    }

    fn query(&self) -> event_store::Query {
        product_query(&self.0.product_id)
    }

    fn apply(&mut self, event: &event_store::SequencedEvent) -> Result<(), Self::Error> {
        self.0.apply(&ExistenceReducers, event)
    }
}
```

The loader restores the snapshot when it deserializes into the state type,
replays events from `snapshot.position + 1`, and writes a new snapshot when
`SnapshotStore::should_snapshot` allows it. Snapshot writes are best-effort;
the event stream remains the source of truth.

```rust
let state = StateLoader::new(&event_store, &snapshot_store)
    .load(ProductExistenceState::new(product_id))
    .await?;
```
