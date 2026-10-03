-- Snapshot store table persisting model snapshots folded up to a sequence position
CREATE TABLE snapshots (
  -- Unique string identifier for the snapshot state acting as the primary key (e.g. "order:123", "account:1")
  key TEXT PRIMARY KEY,
  -- Position of the last event folded into the snapshot state; NULL if no events folded
  position BIGINT,
  -- Serialized JSON model state folded up to the given position
  state JSONB NOT NULL,
  -- Timestamp when the snapshot entry was created
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  -- Timestamp when the snapshot entry was last updated
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
