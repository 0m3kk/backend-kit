# projection

Reusable projection primitives for event-sourced services.

## Migrations

`PostgresCheckpointStore` requires a `projection_checkpoints` table, which ships
as an embedded migration. `PostgresCheckpointStore::init(&pool)` runs the
migration and constructs the store in one step. To migrate without building a
store, call `PostgresCheckpointStore::migrate(&pool)` or
`projection::migrator().run(&pool)`.

`migrator()` records applied migrations in `_projection_migrations`, so it does
not interfere with an application's own migrations in `_sqlx_migrations`.
`PROJECTION_MIGRATOR` is available for tests that use `#[sqlx::test]`.

## View contract

Implement [`View`](src/view.rs) for each persisted read model. A view declares:

- a stable `name` used as its checkpoint key;
- the event-store `query` it consumes;
- an `apply` method that maps one `SequencedEvent` onto its table.

## Catch-up worker

`ProjectionWorker` reads the checkpoint, streams matching events, applies them,
and advances the checkpoint every 100 events and at the end of a run. Crash
recovery stays safe when the view also guards writes by event position.

## Catch-up loop

`run_catchup_loop` drives a worker until a shutdown future completes. The first
run happens immediately, then once per configured interval. Errors are logged
and the loop keeps going, so a transient database failure does not stop the
worker.

```rust
use std::time::Duration;

use projection::{CatchupConfig, run_catchup_loop};

let config = CatchupConfig::from_env("PROJECTION_WORKER_INTERVAL_SECONDS");
let shutdown = async {
    let _ = tokio::signal::ctrl_c().await;
};

run_catchup_loop(&worker, &view, config, shutdown).await;
```

`CatchupConfig::from_env` reads whole seconds. Missing, invalid, and zero values
fall back to `DEFAULT_CATCHUP_INTERVAL` (5 seconds).
