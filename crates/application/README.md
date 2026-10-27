# application

Generic application-layer primitives shared by backend services.

Currently provides:

- [`ExecutionContext`](src/context.rs), which carries request metadata (actor,
  correlation, tracing, client details) and converts domain events into
  `event-store` events enriched with that metadata.
- [`State`](src/state.rs), [`StateSet`](src/state.rs), and
  [`StateLoader`](src/state.rs), which rehydrate decision states from snapshots
  and newer events and derive the matching append condition.

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
        // Exactly the events the reducer below consumes.
        event_store::Query::from_items([event_store::QueryItem::new(
            vec!["product_created".to_string(), "product_deleted".to_string()],
            vec![format!("product:{}", self.0.product_id)],
        )])
    }

    fn apply(&mut self, event: &event_store::SequencedEvent) -> Result<(), Self::Error> {
        self.0.apply(&ExistenceReducers, event)
    }
}
```

The loader restores snapshots when they deserialize into the state type, reads
the events after the oldest snapshot, and writes a new snapshot when
`SnapshotStore::should_snapshot` allows it. Snapshot writes are best-effort; the
event stream remains the source of truth.

`load` returns a `Decision` that carries the combined query and the last
observed position:

```rust
let decision = StateLoader::new(&event_store, &snapshot_store)
    .load(ProductExistenceState::new(product_id))
    .await?;

decision.states().ensure_available()?;
event_store
    .append(events, decision.append_condition())
    .await?;
```

## Loading several states

`load_set` accepts a tuple of states that share an error type. It queries the
union of their queries once, then dispatches each event only to the states whose
query matches it and that have not already folded it through a snapshot:

```rust
let decision = StateLoader::new(&event_store, &snapshot_store)
    .load_set((
        ProductExistenceState::new(product_id.clone()),
        ProductDetailsState::new(product_id.clone()),
    ))
    .await?;

let (existence, details) = decision.states();
existence.ensure_available()?;
```

Because every state came from the same read, the combined query plus the
observed position is a sound append condition:

```rust
event_store
    .append(events, decision.append_condition())
    .await?;
```

Keep each state's query equal to the events its reducer reads. A broader query
still protects correctness but produces false conflicts; a narrower one can
miss a concurrent change.
