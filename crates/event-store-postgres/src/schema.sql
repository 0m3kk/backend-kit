CREATE TABLE IF NOT EXISTS events (
  id VARCHAR(255) NOT NULL,
  position bigserial PRIMARY KEY,
  event_type VARCHAR(255) NOT NULL,
  data JSONB NOT NULL,
  tags TEXT[] NOT NULL DEFAULT '{}',
  metadata JSONB,
  TIMESTAMP BIGINT NOT NULL
);


-- Unique index to enforce globally unique event IDs and speed up single-event lookups
CREATE UNIQUE INDEX if NOT EXISTS idx_events_id ON events (id);


-- Composite index for filtering by event_type and ordering by sequence position
CREATE INDEX if NOT EXISTS idx_events_event_type_position ON events (event_type, position);


-- GIN index for fast tag containment matching (tags @> $1)
CREATE INDEX if NOT EXISTS idx_events_tags ON events USING gin (tags);
