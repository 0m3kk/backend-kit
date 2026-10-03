use std::time::Instant;

use async_trait::async_trait;
use opentelemetry::global;
use opentelemetry::metrics::{Counter, Histogram};
use sqlx::{PgPool, Row};

use crate::cipher::CryptoError;
use crate::envelope::{EnvelopeData, decrypt_envelope, encrypt_envelope, reencrypt_dek};
use crate::error::{DecryptError, Error};
use crate::keyring::Keyring;
use crate::store::SecretStore;
use crate::value::SecretBytes;

const ROTATE_BATCH_SIZE: i64 = 1000;

/// Embedded database migrations.
pub static SECRET_STORE_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
struct SecretStoreMetrics {
    puts_total: Counter<u64>,
    put_duration: Histogram<f64>,
    gets_total: Counter<u64>,
    get_duration: Histogram<f64>,
    deletes_total: Counter<u64>,
    delete_duration: Histogram<f64>,
    lists_total: Counter<u64>,
    list_duration: Histogram<f64>,
    rotations_total: Counter<u64>,
    secrets_rotated: Counter<u64>,
    rotate_duration: Histogram<f64>,
}

impl Default for SecretStoreMetrics {
    fn default() -> Self {
        let meter = global::meter("secret-store");
        Self {
            puts_total: meter.u64_counter("db_secret_store_puts_total").build(),
            put_duration: meter
                .f64_histogram("db_secret_store_put_duration_seconds")
                .with_unit("s")
                .build(),
            gets_total: meter.u64_counter("db_secret_store_gets_total").build(),
            get_duration: meter
                .f64_histogram("db_secret_store_get_duration_seconds")
                .with_unit("s")
                .build(),
            deletes_total: meter.u64_counter("db_secret_store_deletes_total").build(),
            delete_duration: meter
                .f64_histogram("db_secret_store_delete_duration_seconds")
                .with_unit("s")
                .build(),
            lists_total: meter.u64_counter("db_secret_store_lists_total").build(),
            list_duration: meter
                .f64_histogram("db_secret_store_list_duration_seconds")
                .with_unit("s")
                .build(),
            rotations_total: meter.u64_counter("db_secret_store_rotations_total").build(),
            secrets_rotated: meter
                .u64_counter("db_secret_store_secrets_rotated_total")
                .build(),
            rotate_duration: meter
                .f64_histogram("db_secret_store_rotate_duration_seconds")
                .with_unit("s")
                .build(),
        }
    }
}

/// A PostgreSQL-backed secret store.
#[derive(Debug, Clone)]
pub struct PostgresSecretStore {
    pool: PgPool,
}

impl PostgresSecretStore {
    /// Create a new PostgreSQL secret store with the given connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SecretStore for PostgresSecretStore {
    type Error = sqlx::Error;

    async fn put(
        &self,
        keyring: &Keyring,
        name: &str,
        value: &SecretBytes,
    ) -> Result<(), Error<Self::Error>> {
        let start = Instant::now();
        let result = put_inner(&self.pool, keyring, name, value).await;
        if result.is_ok() {
            let metrics = SecretStoreMetrics::default();
            metrics.puts_total.add(1, &[]);
            metrics
                .put_duration
                .record(start.elapsed().as_secs_f64(), &[]);
        }
        result
    }

    async fn get(
        &self,
        keyring: &Keyring,
        name: &str,
    ) -> Result<Option<SecretBytes>, Error<Self::Error>> {
        let start = Instant::now();
        let row = sqlx::query(
            "SELECT payload_ciphertext, payload_nonce, encrypted_dek, dek_nonce, dek_salt, kek_version \
             FROM secrets WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Store)?;

        let result = decrypt_row(keyring, name, row);
        if result.is_ok() {
            let metrics = SecretStoreMetrics::default();
            metrics.gets_total.add(1, &[]);
            metrics
                .get_duration
                .record(start.elapsed().as_secs_f64(), &[]);
        }
        result
    }

    async fn delete(&self, name: &str) -> Result<bool, Error<Self::Error>> {
        let start = Instant::now();
        let row = sqlx::query("DELETE FROM secrets WHERE name = $1 RETURNING name")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(Error::Store)?;

        let metrics = SecretStoreMetrics::default();
        metrics.deletes_total.add(1, &[]);
        metrics
            .delete_duration
            .record(start.elapsed().as_secs_f64(), &[]);

        Ok(row.is_some())
    }

    async fn list_names(&self) -> Result<Vec<String>, Error<Self::Error>> {
        let start = Instant::now();
        let rows = sqlx::query("SELECT name FROM secrets ORDER BY name")
            .fetch_all(&self.pool)
            .await
            .map_err(Error::Store)?;

        let metrics = SecretStoreMetrics::default();
        metrics.lists_total.add(1, &[]);
        metrics
            .list_duration
            .record(start.elapsed().as_secs_f64(), &[]);

        Ok(rows
            .into_iter()
            .map(|row| row.get::<String, _>("name"))
            .collect())
    }

    async fn rotate_keys(&self, keyring: &Keyring) -> Result<usize, Error<Self::Error>> {
        let start = Instant::now();
        let target_version = i32::try_from(keyring.active_version()).unwrap_or(i32::MAX);
        let mut total_rotated = 0;

        loop {
            let mut tx = self.pool.begin().await.map_err(Error::Store)?;
            let rows = sqlx::query(
                "SELECT name, encrypted_dek, dek_nonce, dek_salt, kek_version \
                 FROM secrets \
                 WHERE kek_version < $1 \
                 ORDER BY name \
                 LIMIT $2 \
                 FOR UPDATE",
            )
            .bind(target_version)
            .bind(ROTATE_BATCH_SIZE)
            .fetch_all(&mut *tx)
            .await
            .map_err(Error::Store)?;

            if rows.is_empty() {
                let _ = tx.rollback().await;
                break;
            }

            let batch_count = rows.len();
            for row in rows {
                let name: String = row.get("name");
                let kek_version: i32 = row.get("kek_version");
                let version = u32::try_from(kek_version).unwrap_or(0);
                let Some(old_kek) = keyring.get(version) else {
                    let _ = tx.rollback().await;
                    return Err(Error::MissingKeyVersion { name, version });
                };

                let encrypted_dek: Vec<u8> = row.get("encrypted_dek");
                let dek_nonce: Vec<u8> = row.get("dek_nonce");
                let dek_salt: Option<Vec<u8>> = row.get("dek_salt");
                let dek_nonce = dek_nonce.try_into().unwrap_or([0u8; 12]);
                let dek_salt = dek_salt.map(|s| s.try_into().unwrap_or([0u8; 16]));

                let new_dek_enc = match reencrypt_dek(
                    old_kek,
                    keyring.active_cipher(),
                    &encrypted_dek,
                    dek_nonce,
                    dek_salt,
                ) {
                    Ok(enc) => enc,
                    Err(CryptoError::Decrypt) => {
                        let _ = tx.rollback().await;
                        return Err(Error::Decrypt(DecryptError { name }));
                    }
                    Err(other) => {
                        let _ = tx.rollback().await;
                        return Err(crypto_to_store(other));
                    }
                };

                sqlx::query(
                    "UPDATE secrets \
                     SET encrypted_dek = $1, dek_nonce = $2, dek_salt = $3, kek_version = $4, updated_at = NOW() \
                     WHERE name = $5",
                )
                .bind(new_dek_enc.ciphertext)
                .bind(new_dek_enc.nonce)
                .bind(new_dek_enc.salt)
                .bind(target_version)
                .bind(&name)
                .execute(&mut *tx)
                .await
                .map_err(Error::Store)?;
            }

            tx.commit().await.map_err(Error::Store)?;
            total_rotated += batch_count;

            if (batch_count as i64) < ROTATE_BATCH_SIZE {
                break;
            }
        }

        let metrics = SecretStoreMetrics::default();
        metrics.rotations_total.add(1, &[]);
        metrics.secrets_rotated.add(total_rotated as u64, &[]);
        metrics
            .rotate_duration
            .record(start.elapsed().as_secs_f64(), &[]);

        Ok(total_rotated)
    }
}

fn crypto_to_store(e: CryptoError) -> Error<sqlx::Error> {
    Error::Store(sqlx::Error::Protocol(format!("crypto error: {e}")))
}

fn row_to_envelope(row: &sqlx::postgres::PgRow) -> EnvelopeData {
    let payload_ciphertext: Vec<u8> = row.get("payload_ciphertext");
    let payload_nonce: Vec<u8> = row.get("payload_nonce");
    let encrypted_dek: Vec<u8> = row.get("encrypted_dek");
    let dek_nonce: Vec<u8> = row.get("dek_nonce");
    let dek_salt: Option<Vec<u8>> = row.get("dek_salt");

    let payload_nonce = payload_nonce.try_into().unwrap_or([0u8; 12]);
    let dek_nonce = dek_nonce.try_into().unwrap_or([0u8; 12]);
    let dek_salt = dek_salt.map(|s| s.try_into().unwrap_or([0u8; 16]));

    EnvelopeData {
        payload_ciphertext,
        payload_nonce,
        encrypted_dek,
        dek_nonce,
        dek_salt,
    }
}

fn decrypt_row(
    keyring: &Keyring,
    name: &str,
    row: Option<sqlx::postgres::PgRow>,
) -> Result<Option<SecretBytes>, Error<sqlx::Error>> {
    let Some(row) = row else {
        return Ok(None);
    };

    let kek_version: i32 = row.get("kek_version");
    let version = u32::try_from(kek_version).unwrap_or(0);
    let Some(kek) = keyring.get(version) else {
        return Err(Error::MissingKeyVersion {
            name: name.to_string(),
            version,
        });
    };

    let env = row_to_envelope(&row);
    match decrypt_envelope(kek, &env) {
        Ok(plain) => Ok(Some(plain)),
        Err(CryptoError::Decrypt) => Err(Error::Decrypt(DecryptError {
            name: name.to_string(),
        })),
        Err(other) => Err(crypto_to_store(other)),
    }
}

async fn put_inner<'a, E>(
    executor: E,
    keyring: &Keyring,
    name: &str,
    value: &SecretBytes,
) -> Result<(), Error<sqlx::Error>>
where
    E: sqlx::Executor<'a, Database = sqlx::Postgres>,
{
    let env = encrypt_envelope(keyring.active_cipher(), value).map_err(crypto_to_store)?;
    let kek_version = i32::try_from(keyring.active_version()).unwrap_or(i32::MAX);

    sqlx::query(
        "INSERT INTO secrets (name, payload_ciphertext, payload_nonce, encrypted_dek, dek_nonce, dek_salt, kek_version, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW()) \
         ON CONFLICT (name) DO UPDATE SET \
           payload_ciphertext = EXCLUDED.payload_ciphertext, \
           payload_nonce = EXCLUDED.payload_nonce, \
           encrypted_dek = EXCLUDED.encrypted_dek, \
           dek_nonce = EXCLUDED.dek_nonce, \
           dek_salt = EXCLUDED.dek_salt, \
           kek_version = EXCLUDED.kek_version, \
           updated_at = NOW()",
    )
    .bind(name)
    .bind(env.payload_ciphertext)
    .bind(env.payload_nonce)
    .bind(env.encrypted_dek)
    .bind(env.dek_nonce)
    .bind(env.dek_salt)
    .bind(kek_version)
    .execute(executor)
    .await
    .map_err(Error::Store)?;

    Ok(())
}
