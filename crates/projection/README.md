# projection

Reusable projection primitives for event-sourced services: a multi-view
supervisor, an exactly-once batch contract, PostgreSQL checkpoints, health, and
leader election.

## Migrations

`PostgresCheckpointStore` requires a `projection_checkpoints` table, which ships
as an embedded migration. `PostgresCheckpointStore::init(&pool)` runs the
migration and constructs the store in one step. To migrate without building a
store, call `PostgresCheckpointStore::migrate(&pool)` or
`projection::migrator().run(&pool)`.

`migrator()` records applied migrations in `_projection_migrations`, so it does
not interfere with an application's own migrations in `_sqlx_migrations`.
`PROJECTION_MIGRATOR` is available for tests that use `#[sqlx::test]`.

## Projection contract

Implement [`Projection`](src/projection.rs) for each read model. A projection
declares:

- a stable `name` used as its checkpoint key;
- the event-store `query` it consumes;
- `apply_batch`, which writes the rows and the checkpoint;
- `reset`, which clears the view for a rebuild.

`apply_batch` receives the highest event position of the batch. It must persist
the rows and the checkpoint in the same transaction, so a batch is
exactly-once: a crash commits either both or neither. PostgreSQL implementations
use `save_in_transaction`:

```rust
use projection::{ProjectionError, ProjectionFuture, save_in_transaction};

impl Projection for PostgresProductListView {
    fn apply_batch<'a>(
        &'a self,
        events: &'a [SequencedEvent],
        checkpoint: SequencePosition,
    ) -> ProjectionFuture<'a, Result<(), ProjectionError>> {
        Box::pin(async move {
            let mut tx = self.pool.begin().await.map_err(store_error)?;
            for event in events {
                apply_event(&mut tx, event).await.map_err(store_error)?;
            }
            save_in_transaction(&mut tx, self.name(), checkpoint)
                .await
                .map_err(store_error)?;
            tx.commit().await.map_err(store_error)?;
            Ok(())
        })
    }

    // name(), query(), reset() ...
}
```

Row handlers must stay idempotent as well: guard writes with
`last_event_position < EXCLUDED.last_event_position` so replay after a crash
cannot regress a row.

## Multi-view supervisor

`ProjectionSupervisor` runs one catch-up loop per `Projection`. It owns the
wake channel fan-out, per-view checkpoints, health state, and shutdown
behavior.

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

Each loop:

- polls on the configured interval (default 5 seconds);
- drains full batches back-to-back, so a large backlog is not limited to one
  batch per tick;
- checks shutdown between batches, so an in-flight batch always commits
  atomically instead of being cancelled half-way;
- treats notifications as hints; a lost or closed notification channel leaves
  the polling ticker as the safety net.

`ProjectionSupervisor::catchup_once(name)` runs a single pass for tests and
one-shot drains. `rebuild(name)` resets the checkpoint first and then the view,
so a crash between the two operations replays against old rows instead of
leaving an empty view with an advanced checkpoint.
`WorkerHealth::snapshot()` reports the last applied position, processed count,
lag, and consecutive errors per view.

## Metrics

The supervisor emits OpenTelemetry metrics through the `telemetry` crate:

| Metric | Type | Labels | Meaning |
| --- | --- | --- | --- |
| `projection_processed_total` | counter | `view` | Events applied by this process |
| `projection_errors_total` | counter | `view` | Failed catch-up attempts |
| `projection_last_position` | gauge | `view` | Highest applied event position |
| `projection_lag_seconds` | gauge | `view` | Age of the oldest event in the last applied batch; 0 when caught up |

Lag uses the event metadata timestamp, so events written with
`ExecutionContext::create_store_event` report lag out of the box. Events
without typed metadata record zero.

## Health and readiness

`WorkerHealth::is_healthy()` is false after any failed attempt.
`WorkerHealth::is_ready()` tolerates transient errors and turns false after
`UNHEALTHY_AFTER_ERRORS` (3) consecutive failures for a view. Expose it from a
`/readyz` endpoint and keep `/healthz` as a liveness probe.

## Wake-ups and notifications

The `event-store` crate installs an `AFTER INSERT` trigger that emits
`NOTIFY events_appended`, and `spawn_listener` turns those notifications into
wake signals:

```rust
use tokio::sync::{broadcast, watch};

use projection::spawn_listener;

let (wake_tx, _) = broadcast::channel(1);
let listener = spawn_listener(
    pool.clone(),
    event_store::EVENTS_NOTIFY_CHANNEL,
    wake_tx.clone(),
    shutdown_rx.clone(),
);

supervisor.run(wake_tx.subscribe(), shutdown_rx.clone()).await?;

let _ = listener.await;
```

- Notifications are coalesced by the debounce window (default 50 ms).
- The polling ticker remains active as the safety net.
- The listener reconnects with exponential backoff and emits a wake signal after
  reconnecting, because notifications sent while disconnected are lost.
- The listener holds one connection from the pool.

## Leader election

`LeaderGuard` wraps `pg_try_advisory_lock` for active-passive workers:

```rust
use projection::LeaderGuard;

let guard = LeaderGuard::acquire(&pool, lock_id, retry_interval, &mut shutdown_rx).await?;
if guard.is_none() && lock_id > 0 {
    return Ok(()); // shutdown while standing by
}

// ... run the worker loops ...

if let Some(guard) = guard {
    guard.release().await;
}
```

A non-positive `lock_id` disables leader election. The guard keeps a dedicated
connection alive because the lock is session-level. If that connection is lost,
PostgreSQL releases the lock and a standby can take over; size the pool for at
least three concurrent connections (leader lock, notification listener, and
catch-up reads and writes).
