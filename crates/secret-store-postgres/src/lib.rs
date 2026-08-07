use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgPool, Row};
use tokio::sync::RwLock;

use secret_store::{
    CipherAlgorithm, EncryptedPayload, KeyRing, ListSecretOptions, MasterKey, SecretCrypto,
    SecretEntry, SecretError, SecretHeader, SecretPath, SecretStore, SecretStoreTx, SecretValue,
    SetSecretOptions,
};

/// PostgreSQL-backed implementation of `SecretStore` with Envelope Encryption and `KeyRing` rotation.
#[derive(Clone)]
pub struct PostgresSecretStore {
    pool: PgPool,
    keyring: Arc<RwLock<KeyRing>>,
    default_cipher: CipherAlgorithm,
}

impl PostgresSecretStore {
    /// Create a new `PostgresSecretStore`.
    pub fn new(pool: PgPool, keyring: KeyRing, default_cipher: CipherAlgorithm) -> Self {
        Self {
            pool,
            keyring: Arc::new(RwLock::new(keyring)),
            default_cipher,
        }
    }

    /// Execute embedded SQL migrations to create necessary tables and indexes.
    pub async fn migrate(&self) -> Result<(), SecretError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| SecretError::StoreError(e.to_string()))
    }

    /// Add a new `MasterKey` version to the store's `KeyRing`.
    pub async fn add_master_key(&self, key: MasterKey) -> Result<(), SecretError> {
        let mut kr_guard = self.keyring.write().await;
        let mut keys: Vec<MasterKey> = Vec::new();
        let current_ver = kr_guard.current_version();
        for v in 1..=current_ver.max(key.version()) {
            if let Ok(k) = kr_guard.get_key(v) {
                keys.push(k.clone());
            }
        }
        keys.push(key);
        *kr_guard = KeyRing::new(keys)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SecretStoreTx<PgConnection> — the canonical transactional implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl SecretStoreTx<sqlx::PgConnection> for PostgresSecretStore {
    async fn get_tx(
        &self,
        conn: &mut sqlx::PgConnection,
        path: &SecretPath,
    ) -> Result<Option<SecretEntry>, SecretError> {
        let row = sqlx::query(
            "SELECT h.active_version, h.tags, v.cipher, v.kek_version, v.wrapped_dek, v.nonce, v.ciphertext, v.tag, v.created_at, v.expires_at \
             FROM secret_headers h \
             JOIN secret_versions v ON h.path = v.path AND h.active_version = v.version \
             WHERE h.path = $1 AND h.is_deleted = FALSE AND (v.expires_at IS NULL OR v.expires_at > NOW())"
        )
        .bind(path.as_str())
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| SecretError::StoreError(e.to_string()))?;

        let row = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        let version: i64 = row.get("active_version");
        let tags_json: serde_json::Value = row.get("tags");
        let tags: HashMap<String, String> = serde_json::from_value(tags_json)
            .map_err(|e| SecretError::SerializationError(e.to_string()))?;

        let cipher_str: String = row.get("cipher");
        let cipher = match cipher_str.as_str() {
            "Aes256Gcm" => CipherAlgorithm::Aes256Gcm,
            "ChaCha20Poly1305" => CipherAlgorithm::ChaCha20Poly1305,
            other => {
                return Err(SecretError::StoreError(format!(
                    "Unknown cipher in database: {other}"
                )));
            }
        };

        let kek_version_i32: i32 = row.get("kek_version");
        let wrapped_dek: Vec<u8> = row.get("wrapped_dek");
        let nonce: Vec<u8> = row.get("nonce");
        let ciphertext: Vec<u8> = row.get("ciphertext");
        let tag: Option<Vec<u8>> = row.get("tag");
        let created_at_dt: OffsetDateTime = row.get("created_at");
        let expires_at_dt: Option<OffsetDateTime> = row.get("expires_at");

        let payload = EncryptedPayload {
            cipher,
            kek_version: kek_version_i32 as u32,
            wrapped_dek,
            nonce,
            ciphertext,
            tag,
        };

        let keyring_guard = self.keyring.read().await;
        let decrypted_bytes = SecretCrypto::decrypt_envelope(&payload, &keyring_guard)?;

        Ok(Some(SecretEntry {
            path: path.clone(),
            value: SecretValue::new(decrypted_bytes),
            version: version as u64,
            tags,
            created_at: created_at_dt.into(),
            expires_at: expires_at_dt.map(Into::into),
        }))
    }

    async fn get_version_tx(
        &self,
        conn: &mut sqlx::PgConnection,
        path: &SecretPath,
        version: u64,
    ) -> Result<Option<SecretEntry>, SecretError> {
        let row = sqlx::query(
            "SELECT h.tags, v.cipher, v.kek_version, v.wrapped_dek, v.nonce, v.ciphertext, v.tag, v.created_at, v.expires_at \
             FROM secret_headers h \
             JOIN secret_versions v ON h.path = v.path AND v.version = $2 \
             WHERE h.path = $1 AND (v.expires_at IS NULL OR v.expires_at > NOW())"
        )
        .bind(path.as_str())
        .bind(version as i64)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| SecretError::StoreError(e.to_string()))?;

        let row = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        let tags_json: serde_json::Value = row.get("tags");
        let tags: HashMap<String, String> = serde_json::from_value(tags_json)
            .map_err(|e| SecretError::SerializationError(e.to_string()))?;

        let cipher_str: String = row.get("cipher");
        let cipher = match cipher_str.as_str() {
            "Aes256Gcm" => CipherAlgorithm::Aes256Gcm,
            "ChaCha20Poly1305" => CipherAlgorithm::ChaCha20Poly1305,
            other => {
                return Err(SecretError::StoreError(format!(
                    "Unknown cipher in database: {other}"
                )));
            }
        };

        let kek_version_i32: i32 = row.get("kek_version");
        let wrapped_dek: Vec<u8> = row.get("wrapped_dek");
        let nonce: Vec<u8> = row.get("nonce");
        let ciphertext: Vec<u8> = row.get("ciphertext");
        let tag: Option<Vec<u8>> = row.get("tag");
        let created_at_dt: OffsetDateTime = row.get("created_at");
        let expires_at_dt: Option<OffsetDateTime> = row.get("expires_at");

        let payload = EncryptedPayload {
            cipher,
            kek_version: kek_version_i32 as u32,
            wrapped_dek,
            nonce,
            ciphertext,
            tag,
        };

        let keyring_guard = self.keyring.read().await;
        let decrypted_bytes = SecretCrypto::decrypt_envelope(&payload, &keyring_guard)?;

        Ok(Some(SecretEntry {
            path: path.clone(),
            value: SecretValue::new(decrypted_bytes),
            version,
            tags,
            created_at: created_at_dt.into(),
            expires_at: expires_at_dt.map(Into::into),
        }))
    }

    async fn set_tx(
        &self,
        conn: &mut sqlx::PgConnection,
        path: SecretPath,
        value: SecretValue,
        options: SetSecretOptions,
    ) -> Result<SecretEntry, SecretError> {
        let keyring_guard = self.keyring.read().await;
        let payload =
            SecretCrypto::encrypt_envelope(self.default_cipher, &keyring_guard, value.as_bytes())?;

        let tags_json = serde_json::to_value(&options.tags)
            .map_err(|e| SecretError::SerializationError(e.to_string()))?;

        let ttl_secs = options.ttl.map(|d| d.as_secs_f64());

        let header_row = sqlx::query(
            "INSERT INTO secret_headers (path, active_version, max_version, tags, is_deleted, updated_at) \
             VALUES ($1, 1, 1, $2, FALSE, NOW()) \
             ON CONFLICT (path) DO UPDATE SET \
               active_version = secret_headers.max_version + 1, \
               max_version = secret_headers.max_version + 1, \
               tags = CASE WHEN $2 = '{}'::jsonb THEN secret_headers.tags ELSE EXCLUDED.tags END, \
               is_deleted = FALSE, \
               updated_at = NOW() \
             RETURNING active_version, tags"
        )
        .bind(path.as_str())
        .bind(&tags_json)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| SecretError::StoreError(e.to_string()))?;

        let version_i64: i64 = header_row.get("active_version");
        let effective_tags_json: serde_json::Value = header_row.get("tags");
        let effective_tags: HashMap<String, String> =
            serde_json::from_value(effective_tags_json)
                .map_err(|e| SecretError::SerializationError(e.to_string()))?;

        let cipher_str = match payload.cipher {
            CipherAlgorithm::Aes256Gcm => "Aes256Gcm",
            CipherAlgorithm::ChaCha20Poly1305 => "ChaCha20Poly1305",
        };

        let ver_row = sqlx::query(
            "INSERT INTO secret_versions (path, version, cipher, kek_version, wrapped_dek, nonce, ciphertext, tag, created_at, expires_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), CASE WHEN $9::FLOAT IS NOT NULL THEN NOW() + ($9 || ' seconds')::INTERVAL ELSE NULL END) \
             RETURNING created_at, expires_at"
        )
        .bind(path.as_str())
        .bind(version_i64)
        .bind(cipher_str)
        .bind(payload.kek_version as i32)
        .bind(&payload.wrapped_dek)
        .bind(&payload.nonce)
        .bind(&payload.ciphertext)
        .bind(&payload.tag)
        .bind(ttl_secs)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| SecretError::StoreError(e.to_string()))?;

        let created_at_dt: OffsetDateTime = ver_row.get("created_at");
        let expires_at_dt: Option<OffsetDateTime> = ver_row.get("expires_at");

        Ok(SecretEntry {
            path,
            value,
            version: version_i64 as u64,
            tags: effective_tags,
            created_at: created_at_dt.into(),
            expires_at: expires_at_dt.map(Into::into),
        })
    }

    async fn delete_tx(
        &self,
        conn: &mut sqlx::PgConnection,
        path: &SecretPath,
    ) -> Result<bool, SecretError> {
        let res = sqlx::query(
            "UPDATE secret_headers SET is_deleted = TRUE, updated_at = NOW() WHERE path = $1 AND is_deleted = FALSE"
        )
        .bind(path.as_str())
        .execute(&mut *conn)
        .await
        .map_err(|e| SecretError::StoreError(e.to_string()))?;

        Ok(res.rows_affected() > 0)
    }

    async fn list_tx(
        &self,
        conn: &mut sqlx::PgConnection,
        options: ListSecretOptions,
    ) -> Result<Vec<SecretHeader>, SecretError> {
        let tag_filter_json = serde_json::to_value(&options.tag_filter)
            .map_err(|e| SecretError::SerializationError(e.to_string()))?;

        let mut sql = String::from(
            "SELECT h.path, h.active_version, h.tags, h.is_deleted, v.created_at, v.expires_at \
             FROM secret_headers h \
             JOIN secret_versions v ON h.path = v.path AND h.active_version = v.version \
             WHERE 1=1",
        );

        if !options.include_deleted {
            sql.push_str(
                " AND h.is_deleted = FALSE AND (v.expires_at IS NULL OR v.expires_at > NOW())",
            );
        }

        if let Some(ref prefix) = options.prefix {
            let escaped = prefix.as_str().replace('\'', "''");
            sql.push_str(&format!(" AND h.path LIKE '{escaped}%'"));
        }

        if !options.tag_filter.is_empty() {
            let tag_json_str = tag_filter_json.to_string().replace('\'', "''");
            sql.push_str(&format!(" AND h.tags @> '{tag_json_str}'::jsonb"));
        }

        sql.push_str(" ORDER BY h.path ASC");

        if let Some(limit) = options.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let rows = sqlx::query(&sql)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| SecretError::StoreError(e.to_string()))?;

        let mut headers = Vec::with_capacity(rows.len());

        for row in rows {
            let p_str: String = row.get("path");
            let path = SecretPath::new(p_str)?;
            let active_version: i64 = row.get("active_version");
            let tags_json: serde_json::Value = row.get("tags");
            let tags: HashMap<String, String> = serde_json::from_value(tags_json)
                .map_err(|e| SecretError::SerializationError(e.to_string()))?;
            let is_deleted: bool = row.get("is_deleted");
            let created_at_dt: OffsetDateTime = row.get("created_at");
            let expires_at_dt: Option<OffsetDateTime> = row.get("expires_at");

            headers.push(SecretHeader {
                path,
                version: active_version as u64,
                tags,
                created_at: created_at_dt.into(),
                expires_at: expires_at_dt.map(Into::into),
                is_deleted,
            });
        }

        Ok(headers)
    }

    async fn rotate_key_tx(&self, conn: &mut sqlx::PgConnection) -> Result<u64, SecretError> {
        let keyring_guard = self.keyring.read().await;
        let target_kek_version = keyring_guard.current_version();

        let rows = sqlx::query(
            "SELECT path, version, kek_version, wrapped_dek FROM secret_versions WHERE kek_version != $1"
        )
        .bind(target_kek_version as i32)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| SecretError::StoreError(e.to_string()))?;

        let mut count = 0u64;

        for row in rows {
            let path_str: String = row.get("path");
            let version: i64 = row.get("version");
            let row_kek_version: i32 = row.get("kek_version");
            let wrapped_dek: Vec<u8> = row.get("wrapped_dek");

            let dek = keyring_guard.unwrap_dek(row_kek_version as u32, &wrapped_dek)?;
            let (new_wrapped_dek, new_version) = keyring_guard.wrap_dek(&dek)?;

            sqlx::query(
                "UPDATE secret_versions SET kek_version = $1, wrapped_dek = $2 WHERE path = $3 AND version = $4"
            )
            .bind(new_version as i32)
            .bind(&new_wrapped_dek)
            .bind(&path_str)
            .bind(version)
            .execute(&mut *conn)
            .await
            .map_err(|e| SecretError::StoreError(e.to_string()))?;

            count += 1;
        }

        Ok(count)
    }

    async fn clean_expired_tx(
        &self,
        conn: &mut sqlx::PgConnection,
        limit: Option<usize>,
    ) -> Result<u64, SecretError> {
        let max_limit = limit.unwrap_or(1000) as i64;
        let res = sqlx::query(
            "DELETE FROM secret_versions WHERE (path, version) IN (\
               SELECT path, version FROM secret_versions WHERE expires_at IS NOT NULL AND expires_at <= NOW() LIMIT $1\
             )"
        )
        .bind(max_limit)
        .execute(&mut *conn)
        .await
        .map_err(|e| SecretError::StoreError(e.to_string()))?;

        Ok(res.rows_affected())
    }
}

// ---------------------------------------------------------------------------
// SecretStore — delegates to SecretStoreTx by acquiring a connection from the pool
// ---------------------------------------------------------------------------

#[async_trait]
impl SecretStore for PostgresSecretStore {
    async fn get(&self, path: &SecretPath) -> Result<Option<SecretEntry>, SecretError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| SecretError::StoreError(e.to_string()))?;
        <Self as SecretStoreTx<sqlx::PgConnection>>::get_tx(self, &mut conn, path).await
    }

    async fn get_version(
        &self,
        path: &SecretPath,
        version: u64,
    ) -> Result<Option<SecretEntry>, SecretError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| SecretError::StoreError(e.to_string()))?;
        <Self as SecretStoreTx<sqlx::PgConnection>>::get_version_tx(self, &mut conn, path, version)
            .await
    }

    async fn set(
        &self,
        path: SecretPath,
        value: SecretValue,
        options: SetSecretOptions,
    ) -> Result<SecretEntry, SecretError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| SecretError::StoreError(e.to_string()))?;

        let entry = <Self as SecretStoreTx<sqlx::PgConnection>>::set_tx(
            self, &mut tx, path, value, options,
        )
        .await?;

        tx.commit()
            .await
            .map_err(|e| SecretError::StoreError(e.to_string()))?;

        Ok(entry)
    }

    async fn delete(&self, path: &SecretPath) -> Result<bool, SecretError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| SecretError::StoreError(e.to_string()))?;
        <Self as SecretStoreTx<sqlx::PgConnection>>::delete_tx(self, &mut conn, path).await
    }

    async fn list(&self, options: ListSecretOptions) -> Result<Vec<SecretHeader>, SecretError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| SecretError::StoreError(e.to_string()))?;
        <Self as SecretStoreTx<sqlx::PgConnection>>::list_tx(self, &mut conn, options).await
    }

    async fn rotate_key(&self) -> Result<u64, SecretError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| SecretError::StoreError(e.to_string()))?;

        let count =
            <Self as SecretStoreTx<sqlx::PgConnection>>::rotate_key_tx(self, &mut tx).await?;

        tx.commit()
            .await
            .map_err(|e| SecretError::StoreError(e.to_string()))?;

        Ok(count)
    }

    async fn clean_expired(&self, limit: Option<usize>) -> Result<u64, SecretError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| SecretError::StoreError(e.to_string()))?;
        <Self as SecretStoreTx<sqlx::PgConnection>>::clean_expired_tx(self, &mut conn, limit).await
    }
}
