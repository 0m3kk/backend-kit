use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgPool, Row};

use secret_store::{
    CipherAlgorithm, EncryptedPayload, KeyProvider, ListSecretOptions, SecretCrypto, SecretEntry,
    SecretError, SecretHeader, SecretPath, SecretStore, SecretValue, SetSecretOptions,
};

/// PostgreSQL-backed implementation of `SecretStore`.
#[derive(Clone)]
pub struct PostgresSecretStore {
    pool: PgPool,
    key_provider: Arc<dyn KeyProvider>,
    active_key_id: String,
    default_cipher: CipherAlgorithm,
}

impl PostgresSecretStore {
    /// Create a new `PostgresSecretStore`.
    pub fn new(
        pool: PgPool,
        key_provider: Arc<dyn KeyProvider>,
        active_key_id: impl Into<String>,
        default_cipher: CipherAlgorithm,
    ) -> Self {
        Self {
            pool,
            key_provider,
            active_key_id: active_key_id.into(),
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
}

#[async_trait]
impl SecretStore for PostgresSecretStore {
    async fn get(&self, path: &SecretPath) -> Result<Option<SecretEntry>, SecretError> {
        let row = sqlx::query(
            "SELECT h.active_version, h.tags, v.cipher, v.key_id, v.nonce, v.ciphertext, v.tag, v.created_at, v.expires_at \
             FROM secret_headers h \
             JOIN secret_versions v ON h.path = v.path AND h.active_version = v.version \
             WHERE h.path = $1 AND h.is_deleted = FALSE AND (v.expires_at IS NULL OR v.expires_at > NOW())"
        )
        .bind(path.as_str())
        .fetch_optional(&self.pool)
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

        let key_id: String = row.get("key_id");
        let nonce: Vec<u8> = row.get("nonce");
        let ciphertext: Vec<u8> = row.get("ciphertext");
        let tag: Option<Vec<u8>> = row.get("tag");
        let created_at_dt: OffsetDateTime = row.get("created_at");
        let expires_at_dt: Option<OffsetDateTime> = row.get("expires_at");

        let payload = EncryptedPayload {
            cipher,
            key_id: key_id.clone(),
            nonce,
            ciphertext,
            tag,
        };

        let master_key = self.key_provider.get_key(&key_id)?;
        let decrypted_bytes = SecretCrypto::decrypt(&payload, &master_key)?;

        Ok(Some(SecretEntry {
            path: path.clone(),
            value: SecretValue::new(decrypted_bytes),
            version: version as u64,
            tags,
            created_at: created_at_dt.into(),
            expires_at: expires_at_dt.map(Into::into),
        }))
    }

    async fn get_version(
        &self,
        path: &SecretPath,
        version: u64,
    ) -> Result<Option<SecretEntry>, SecretError> {
        let row = sqlx::query(
            "SELECT h.tags, v.cipher, v.key_id, v.nonce, v.ciphertext, v.tag, v.created_at, v.expires_at \
             FROM secret_headers h \
             JOIN secret_versions v ON h.path = v.path AND v.version = $2 \
             WHERE h.path = $1 AND (v.expires_at IS NULL OR v.expires_at > NOW())"
        )
        .bind(path.as_str())
        .bind(version as i64)
        .fetch_optional(&self.pool)
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

        let key_id: String = row.get("key_id");
        let nonce: Vec<u8> = row.get("nonce");
        let ciphertext: Vec<u8> = row.get("ciphertext");
        let tag: Option<Vec<u8>> = row.get("tag");
        let created_at_dt: OffsetDateTime = row.get("created_at");
        let expires_at_dt: Option<OffsetDateTime> = row.get("expires_at");

        let payload = EncryptedPayload {
            cipher,
            key_id: key_id.clone(),
            nonce,
            ciphertext,
            tag,
        };

        let master_key = self.key_provider.get_key(&key_id)?;
        let decrypted_bytes = SecretCrypto::decrypt(&payload, &master_key)?;

        Ok(Some(SecretEntry {
            path: path.clone(),
            value: SecretValue::new(decrypted_bytes),
            version,
            tags,
            created_at: created_at_dt.into(),
            expires_at: expires_at_dt.map(Into::into),
        }))
    }

    async fn set(
        &self,
        path: SecretPath,
        value: SecretValue,
        options: SetSecretOptions,
    ) -> Result<SecretEntry, SecretError> {
        let master_key = self.key_provider.get_key(&self.active_key_id)?;
        let payload = SecretCrypto::encrypt(
            self.default_cipher,
            &self.active_key_id,
            &master_key,
            value.as_bytes(),
        )?;

        let tags_json = serde_json::to_value(&options.tags)
            .map_err(|e| SecretError::SerializationError(e.to_string()))?;

        let ttl_secs = options.ttl.map(|d| d.as_secs_f64());

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| SecretError::StoreError(e.to_string()))?;

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
        .fetch_one(&mut *tx)
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
            "INSERT INTO secret_versions (path, version, cipher, key_id, nonce, ciphertext, tag, created_at, expires_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), CASE WHEN $8::FLOAT IS NOT NULL THEN NOW() + ($8 || ' seconds')::INTERVAL ELSE NULL END) \
             RETURNING created_at, expires_at"
        )
        .bind(path.as_str())
        .bind(version_i64)
        .bind(cipher_str)
        .bind(&payload.key_id)
        .bind(&payload.nonce)
        .bind(&payload.ciphertext)
        .bind(&payload.tag)
        .bind(ttl_secs)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| SecretError::StoreError(e.to_string()))?;

        let created_at_dt: OffsetDateTime = ver_row.get("created_at");
        let expires_at_dt: Option<OffsetDateTime> = ver_row.get("expires_at");

        tx.commit()
            .await
            .map_err(|e| SecretError::StoreError(e.to_string()))?;

        Ok(SecretEntry {
            path,
            value,
            version: version_i64 as u64,
            tags: effective_tags,
            created_at: created_at_dt.into(),
            expires_at: expires_at_dt.map(Into::into),
        })
    }

    async fn delete(&self, path: &SecretPath) -> Result<bool, SecretError> {
        let res = sqlx::query(
            "UPDATE secret_headers SET is_deleted = TRUE, updated_at = NOW() WHERE path = $1 AND is_deleted = FALSE"
        )
        .bind(path.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| SecretError::StoreError(e.to_string()))?;

        Ok(res.rows_affected() > 0)
    }

    async fn list(&self, options: ListSecretOptions) -> Result<Vec<SecretHeader>, SecretError> {
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
            .fetch_all(&self.pool)
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

    async fn rotate_key(&self, old_key_id: &str, new_key_id: &str) -> Result<u64, SecretError> {
        let old_master_key = self.key_provider.get_key(old_key_id)?;
        let new_master_key = self.key_provider.get_key(new_key_id)?;

        let rows = sqlx::query(
            "SELECT path, version, cipher, nonce, ciphertext, tag FROM secret_versions WHERE key_id = $1"
        )
        .bind(old_key_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SecretError::StoreError(e.to_string()))?;

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| SecretError::StoreError(e.to_string()))?;

        let mut count = 0u64;

        for row in rows {
            let path_str: String = row.get("path");
            let version: i64 = row.get("version");
            let cipher_str: String = row.get("cipher");
            let nonce: Vec<u8> = row.get("nonce");
            let ciphertext: Vec<u8> = row.get("ciphertext");
            let tag: Option<Vec<u8>> = row.get("tag");

            let cipher = match cipher_str.as_str() {
                "Aes256Gcm" => CipherAlgorithm::Aes256Gcm,
                "ChaCha20Poly1305" => CipherAlgorithm::ChaCha20Poly1305,
                other => return Err(SecretError::StoreError(format!("Unknown cipher: {other}"))),
            };

            let payload = EncryptedPayload {
                cipher,
                key_id: old_key_id.to_string(),
                nonce,
                ciphertext,
                tag,
            };

            let decrypted = SecretCrypto::decrypt(&payload, &old_master_key)?;
            let re_encrypted =
                SecretCrypto::encrypt(cipher, new_key_id, &new_master_key, &decrypted)?;

            sqlx::query(
                "UPDATE secret_versions SET key_id = $1, nonce = $2, ciphertext = $3, tag = $4 WHERE path = $5 AND version = $6"
            )
            .bind(new_key_id)
            .bind(&re_encrypted.nonce)
            .bind(&re_encrypted.ciphertext)
            .bind(&re_encrypted.tag)
            .bind(&path_str)
            .bind(version)
            .execute(&mut *tx)
            .await
            .map_err(|e| SecretError::StoreError(e.to_string()))?;

            count += 1;
        }

        tx.commit()
            .await
            .map_err(|e| SecretError::StoreError(e.to_string()))?;

        Ok(count)
    }

    async fn clean_expired(&self, limit: Option<usize>) -> Result<u64, SecretError> {
        let max_limit = limit.unwrap_or(1000) as i64;
        let res = sqlx::query(
            "DELETE FROM secret_versions WHERE (path, version) IN (\
               SELECT path, version FROM secret_versions WHERE expires_at IS NOT NULL AND expires_at <= NOW() LIMIT $1\
             )"
        )
        .bind(max_limit)
        .execute(&self.pool)
        .await
        .map_err(|e| SecretError::StoreError(e.to_string()))?;

        Ok(res.rows_affected())
    }
}
