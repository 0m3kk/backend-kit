# projection

Reusable projection primitives for event-sourced services.

## Table

`PostgresCheckpointStore` expects a `projection_checkpoints` table:

```sql
CREATE TABLE IF NOT EXISTS projection_checkpoints (
    view_name           TEXT PRIMARY KEY,
    last_event_position BIGINT NOT NULL DEFAULT 0,
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

## View contract

Implement [`View`](src/view.rs) for each persisted read model. A view declares:

- a stable `name` used as its checkpoint key;
- the event-store `query` it consumes;
- an `apply` method that maps one `SequencedEvent` onto its table.

## Catch-up worker

`ProjectionWorker` reads the checkpoint, streams matching events, applies them,
and advances the checkpoint every 100 events and at the end of a run. Crash
recovery stays safe when the view also guards writes by event position.
