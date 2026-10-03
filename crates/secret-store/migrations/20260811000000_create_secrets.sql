-- Secret store table persisting encrypted secrets with envelope encryption
CREATE TABLE secrets (
  -- Unique string identifier for the secret acting as the primary key (e.g. "db/password")
  name TEXT PRIMARY KEY,
  -- Encrypted secret payload bytes (encrypted via DEK using AES-256-GCM)
  payload_ciphertext bytea NOT NULL,
  -- Cryptographic nonce used for encrypting the secret payload
  payload_nonce bytea NOT NULL,
  -- Per-secret Data Encryption Key (DEK) encrypted with the active Master KEK
  encrypted_dek bytea NOT NULL,
  -- Cryptographic nonce used for encrypting the DEK with the KEK
  dek_nonce bytea NOT NULL,
  -- Optional cryptographic salt used during key derivation
  dek_salt bytea,
  -- Active KEK version used to encrypt the DEK (indexed for zero-downtime key rotation)
  kek_version INTEGER NOT NULL DEFAULT 1,
  -- Timestamp when the secret entry was created
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  -- Timestamp when the secret entry was last updated
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_secrets_kek_version ON secrets (kek_version);
