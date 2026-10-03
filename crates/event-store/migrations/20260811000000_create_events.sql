CREATE SEQUENCE events_position_seq START WITH 0 MINVALUE 0;

-- Central event store table storing all domain events in append-only log format
CREATE TABLE events (
  -- Monotonically increasing 64-bit integer representing global event order and sequence position
  position BIGINT PRIMARY KEY DEFAULT NEXTVAL('events_position_seq'),
  -- Domain event type name identifier (e.g. "order_created", "user_registered")
  event_type TEXT NOT NULL,
  -- Payload containing event specific domain details in JSON format
  data JSONB NOT NULL,
  -- Contextual metadata (e.g. causation_id, correlation_id, actor, timestamp)
  metadata JSONB NOT NULL DEFAULT 'null'::JSONB,
  -- Array of stream/aggregate tags for efficient multi-stream event querying (e.g. "order:123")
  tags TEXT[] NOT NULL
);

CREATE INDEX idx_events_event_type_position ON events (event_type, position);
CREATE INDEX idx_events_tags ON events USING GIN (tags);
