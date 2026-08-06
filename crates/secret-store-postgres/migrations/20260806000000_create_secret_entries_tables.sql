CREATE TABLE IF NOT EXISTS secret_headers (
  path TEXT PRIMARY KEY,
  active_version BIGINT NOT NULL,
  max_version BIGINT NOT NULL,
  tags JSONB NOT NULL DEFAULT '{}'::JSONB,
  is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);


CREATE TABLE IF NOT EXISTS secret_versions (
  path TEXT NOT NULL REFERENCES secret_headers (path) ON DELETE CASCADE,
  version BIGINT NOT NULL,
  cipher TEXT NOT NULL,
  key_id TEXT NOT NULL,
  nonce bytea NOT NULL,
  ciphertext bytea NOT NULL,
  tag bytea,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  expires_at TIMESTAMPTZ,
  PRIMARY KEY (path, version)
);


CREATE INDEX if NOT EXISTS idx_secret_headers_tags ON secret_headers USING gin (tags);


CREATE INDEX if NOT EXISTS idx_secret_versions_expires_at ON secret_versions (expires_at)
WHERE
  expires_at IS NOT NULL;
