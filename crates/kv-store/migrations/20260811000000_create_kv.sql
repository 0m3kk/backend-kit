-- Key-value store table persisting JSON values under string keys
CREATE TABLE kv (
  -- Unique string identifier acting as the primary key
  key TEXT PRIMARY KEY,
  -- JSON payload stored under the key
  value JSONB NOT NULL,
  -- Timestamp when the key was created
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  -- Timestamp when the key-value pair was last updated
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
