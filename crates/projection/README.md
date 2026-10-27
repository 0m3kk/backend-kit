# projection

Reusable projection primitives for event-sourced services.

## Migrations

`PostgresCheckpointStore` requires a `projection_checkpoints` table, which ships
as an embedded migration. Run it when the projection is initialized:

```rust
projection::migrator().run(&pool).await?;
```

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
