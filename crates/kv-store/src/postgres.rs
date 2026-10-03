use std::time::Instant;

use async_trait::async_trait;
use opentelemetry::global;
use opentelemetry::metrics::{Counter, Histogram};
use sqlx::{PgPool, Postgres, Row, Transaction};

use crate::condition::PutCondition;
use crate::error::{Error, PutConditionError};
use crate::store::KvStore;
use crate::value::{ReplacedValue, StoredValue};

/// Embedded database migrations.
///
/// Run them once against the database before using the store, e.g.
/// `KV_STORE_MIGRATOR.run(&pool).await`.
pub static KV_STORE_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
struct KvStoreMetrics {
    puts_total: Counter<u64>,
    put_duration: Histogram<f64>,
    gets_total: Counter<u64>,
    get_duration: Histogram<f64>,
    deletes_total: Counter<u64>,
    delete_duration: Histogram<f64>,
}

impl Default for KvStoreMetrics {
    fn default() -> Self {
        let meter = global::meter("kv-store");
        Self {
            puts_total: meter.u64_counter("db_kv_store_puts_total").build(),
            put_duration: meter
                .f64_histogram("db_kv_store_put_duration_seconds")
                .with_unit("s")
                .build(),
            gets_total: meter.u64_counter("db_kv_store_gets_total").build(),
            get_duration: meter
                .f64_histogram("db_kv_store_get_duration_seconds")
                .with_unit("s")
                .build(),
            deletes_total: meter.u64_counter("db_kv_store_deletes_total").build(),
            delete_duration: meter
                .f64_histogram("db_kv_store_delete_duration_seconds")
                .with_unit("s")
                .build(),
        }
    }
}

/// A PostgreSQL-backed key-value store.
#[derive(Debug, Clone)]
pub struct PostgresKvStore {
    pool: PgPool,
}

impl PostgresKvStore {
    /// Create a new PostgreSQL key-value store with the given connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl KvStore for PostgresKvStore {
    type Error = sqlx::Error;

    async fn get(&self, key: &str) -> Result<Option<StoredValue>, Error<Self::Error>> {
        let start = Instant::now();
        let row = sqlx::query("SELECT key, value, created_at, updated_at FROM kv WHERE key = $1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(Error::Store)?;

        let result = row.map(row_to_stored);
        let metrics = KvStoreMetrics::default();
        metrics.gets_total.add(1, &[]);
        metrics
            .get_duration
            .record(start.elapsed().as_secs_f64(), &[]);

        Ok(result)
    }

    async fn put(
        &self,
        key: &str,
        value: serde_json::Value,
        condition: Option<PutCondition>,
    ) -> Result<ReplacedValue, Error<Self::Error>> {
        let start = Instant::now();
        let mut tx = self.pool.begin().await.map_err(Error::Store)?;
        let result = put_inner(&mut tx, key, value, condition).await;
        match result {
            Ok(replaced) => {
                tx.commit().await.map_err(Error::Store)?;
                let metrics = KvStoreMetrics::default();
                metrics.puts_total.add(1, &[]);
                metrics
                    .put_duration
                    .record(start.elapsed().as_secs_f64(), &[]);
                Ok(replaced)
            }
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }

    async fn delete(&self, key: &str) -> Result<Option<StoredValue>, Error<Self::Error>> {
        let start = Instant::now();
        let row = sqlx::query(
            "DELETE FROM kv WHERE key = $1 RETURNING key, value, created_at, updated_at",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Store)?;

        let result = row.map(row_to_stored);
        let metrics = KvStoreMetrics::default();
        metrics.deletes_total.add(1, &[]);
        metrics
            .delete_duration
            .record(start.elapsed().as_secs_f64(), &[]);

        Ok(result)
    }
}

fn row_to_stored(row: sqlx::postgres::PgRow) -> StoredValue {
    StoredValue {
        key: row.get("key"),
        value: row.get("value"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

async fn put_inner(
    tx: &mut Transaction<'_, Postgres>,
    key: &str,
    value: serde_json::Value,
    condition: Option<PutCondition>,
) -> Result<ReplacedValue, Error<sqlx::Error>> {
    // Serialize puts on the same key; different keys proceed concurrently.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1)::bigint)")
        .bind(key)
        .execute(&mut **tx)
        .await
        .map_err(Error::Store)?;

    let row = sqlx::query("SELECT value FROM kv WHERE key = $1")
        .bind(key)
        .fetch_optional(&mut **tx)
        .await
        .map_err(Error::Store)?;
    let current = row.map(|row| row.get::<serde_json::Value, _>("value"));

    if let Some(condition) = condition {
        let met = match condition {
            PutCondition::Absent => current.is_none(),
            PutCondition::Value(expected) => current.as_ref() == Some(&expected),
        };
        if !met {
            return Err(Error::Condition(PutConditionError { current }));
        }
    }

    // `created_at` is set only on insert; updates keep the original value.
    let row = sqlx::query(
        "INSERT INTO kv (key, value, created_at, updated_at) \
         VALUES ($1, $2, NOW(), NOW()) \
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW() \
         RETURNING key, value, created_at, updated_at",
    )
    .bind(key)
    .bind(&value)
    .fetch_one(&mut **tx)
    .await
    .map_err(Error::Store)?;

    Ok(ReplacedValue {
        key: row.get("key"),
        value: row.get("value"),
        previous: current,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}
