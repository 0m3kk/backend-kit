CREATE TABLE IF NOT EXISTS kv_entries (
  key bytea PRIMARY KEY,
  value bytea NOT NULL,
  expires_at TIMESTAMPTZ
);


CREATE INDEX if NOT EXISTS idx_kv_entries_expires_at ON kv_entries (expires_at)
WHERE
  expires_at IS NOT NULL;
