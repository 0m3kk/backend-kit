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

## Wake-ups and notifications

`run_catchup_loop_with_wake` adds an immediate wake-up source to the loop. The
`event-store` crate installs an `AFTER INSERT` trigger that emits
`NOTIFY events_appended`, and `spawn_listener` turns those notifications into
wake signals:

```rust
use tokio::sync::{broadcast, watch};

use projection::{CatchupConfig, spawn_listener, run_catchup_loop_with_wake};

let (wake_tx, _) = broadcast::channel(1);
let listener = spawn_listener(
    pool.clone(),
    event_store::EVENTS_NOTIFY_CHANNEL,
    wake_tx.clone(),
    shutdown_rx.clone(),
);

let config = CatchupConfig::from_env("PROJECTION_WORKER_INTERVAL_SECONDS")
    .with_debounce(Duration::from_millis(50));

run_catchup_loop_with_wake(
    &worker,
    &view,
    config,
    wake_tx.subscribe(),
    shutdown,
)
.await;

let _ = listener.await;
```

- Notifications are coalesced by the debounce window (default 50 ms).
- The polling ticker remains active as the safety net.
- The listener reconnects with exponential backoff and emits a wake signal after
  reconnecting, because notifications sent while disconnected are lost.
- The listener holds one connection from the pool.
- The worker also holds one connection for the leader lock and uses the pool
  for event reads and checkpoints. Size the pool for at least three concurrent
  connections.

## Multi-view supervisor

`ProjectionSupervisor` runs one catch-up loop per `Projection` and is the
recommended entry point for a worker binary. The starter supplies a
`Vec<Arc<dyn Projection>>` registry; the supervisor owns the shared wake channel,
per-view checkpoints, health state, and shutdown behavior.

```rust
use std::sync::Arc;

use projection::{CatchupConfig, ProjectionSupervisor, WorkerConfig};

let views: Vec<Arc<dyn Projection>> = vec![Arc::new(product_list_view)];
let supervisor = ProjectionSupervisor::new(
    event_store,
    checkpoints,
    views,
    WorkerConfig::new(CatchupConfig::from_env("PROJECTION_WORKER_INTERVAL_SECONDS")),
);

let health = supervisor.health();
supervisor.run(wake_rx, shutdown_rx).await?;
```

Each view implements `Projection`:

- `name()` is the stable checkpoint key;
- `query()` selects the consumed event types;
- `apply` / `apply_batch` writes idempotently;
- `reset()` clears the view table for a rebuild.

`WorkerHealth::snapshot()` reports the last applied position, processed count,
and last error per view. `shutdown_signal()` handles `SIGTERM` and `SIGINT`.
`ProjectionSupervisor::rebuild(name)` resets both the view and its checkpoint,
so the next run replays from the beginning.

## Leader election

`LeaderGuard` wraps `pg_try_advisory_lock` for active-passive workers:

```rust
use projection::LeaderGuard;

let guard = LeaderGuard::acquire(&pool, lock_id, retry_interval, &mut shutdown_rx).await?;
if guard.is_none() && lock_id > 0 {
    return Ok(()); // another worker is active
}

// ... run the worker loops ...

if let Some(guard) = guard {
    guard.release().await;
}
```

A non-positive `lock_id` disables leader election. The guard keeps a dedicated
connection alive because the lock is session-level. If that connection is lost,
PostgreSQL releases the lock and a standby can take over; treat the lock
connection as part of the worker's health and restart the worker if the database
connection is unstable.
