CREATE TABLE IF NOT EXISTS events (
  id VARCHAR(255) NOT NULL,
  position bigserial PRIMARY KEY,
  event_type VARCHAR(255) NOT NULL,
  data JSONB NOT NULL,
  tags TEXT[] NOT NULL DEFAULT '{}',
  metadata JSONB,
  TIMESTAMP BIGINT NOT NULL
);


CREATE UNIQUE INDEX if NOT EXISTS idx_events_id ON events (id);


CREATE INDEX if NOT EXISTS idx_events_event_type_position ON events (event_type, position);


CREATE INDEX if NOT EXISTS idx_events_tags ON events USING gin (tags);
