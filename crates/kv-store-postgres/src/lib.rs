use std::time::Duration;

use async_stream::stream;
use async_trait::async_trait;
use sqlx::{PgPool, Row};

use kv_store::{
    BatchOp, Key, KvEntry, KvError, KvStore, KvStoreTx, KvStream, ScanOptions, SetOptions, Value,
};

/// PostgreSQL backed Key-Value Store implementing the `KvStore` trait.
#[derive(Clone, Debug)]
pub struct PostgresKvStore {
    pool: PgPool,
}

impl PostgresKvStore {
    /// Creates a new `PostgresKvStore` using the provided PostgreSQL pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Setup the database table schema by running embedded migrations.
    pub async fn migrate(&self) -> Result<(), KvError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// KvStoreTx<PgConnection> — the canonical transactional implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl KvStoreTx<sqlx::PgConnection> for PostgresKvStore {
    async fn get_tx(
        &self,
        conn: &mut sqlx::PgConnection,
        key: &Key,
    ) -> Result<Option<Value>, KvError> {
        let row = sqlx::query(
            "SELECT value FROM kv_entries WHERE key = $1 AND (expires_at IS NULL OR expires_at > NOW())",
        )
        .bind(key.as_bytes())
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| KvError::StoreError(e.to_string()))?;

        if let Some(row) = row {
            let val_bytes: Vec<u8> = row.get("value");
            Ok(Some(Value::new(val_bytes)))
        } else {
            Ok(None)
        }
    }

    async fn set_tx(
        &self,
        conn: &mut sqlx::PgConnection,
        key: Key,
        value: Value,
        options: SetOptions,
    ) -> Result<(), KvError> {
        let ttl_secs = options.ttl.map(|d| d.as_secs_f64());

        if options.if_not_exists {
            let res = sqlx::query(
                "INSERT INTO kv_entries (key, value, expires_at) VALUES ($1, $2, CASE WHEN $3::FLOAT IS NOT NULL THEN NOW() + ($3 || ' seconds')::INTERVAL ELSE NULL END) ON CONFLICT (key) DO NOTHING",
            )
            .bind(key.as_bytes())
            .bind(value.as_bytes())
            .bind(ttl_secs)
            .execute(&mut *conn)
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;

            if res.rows_affected() == 0 {
                return Err(KvError::ConditionFailed);
            }
        } else if options.if_exists {
            let res = sqlx::query(
                "UPDATE kv_entries SET value = $2, expires_at = CASE WHEN $3::FLOAT IS NOT NULL THEN NOW() + ($3 || ' seconds')::INTERVAL ELSE NULL END WHERE key = $1 AND (expires_at IS NULL OR expires_at > NOW())",
            )
            .bind(key.as_bytes())
            .bind(value.as_bytes())
            .bind(ttl_secs)
            .execute(&mut *conn)
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;

            if res.rows_affected() == 0 {
                return Err(KvError::ConditionFailed);
            }
        } else {
            sqlx::query(
                "INSERT INTO kv_entries (key, value, expires_at) VALUES ($1, $2, CASE WHEN $3::FLOAT IS NOT NULL THEN NOW() + ($3 || ' seconds')::INTERVAL ELSE NULL END)
                ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, expires_at = EXCLUDED.expires_at",
            )
            .bind(key.as_bytes())
            .bind(value.as_bytes())
            .bind(ttl_secs)
            .execute(&mut *conn)
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        }

        Ok(())
    }

    async fn delete_tx(&self, conn: &mut sqlx::PgConnection, key: &Key) -> Result<bool, KvError> {
        let res = sqlx::query("DELETE FROM kv_entries WHERE key = $1")
            .bind(key.as_bytes())
            .execute(&mut *conn)
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;

        Ok(res.rows_affected() > 0)
    }

    async fn exists_tx(&self, conn: &mut sqlx::PgConnection, key: &Key) -> Result<bool, KvError> {
        let row = sqlx::query(
            "SELECT 1 FROM kv_entries WHERE key = $1 AND (expires_at IS NULL OR expires_at > NOW())",
        )
        .bind(key.as_bytes())
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| KvError::StoreError(e.to_string()))?;

        Ok(row.is_some())
    }

    async fn batch_tx(
        &self,
        conn: &mut sqlx::PgConnection,
        ops: Vec<BatchOp>,
    ) -> Result<(), KvError> {
        for op in ops {
            match op {
                BatchOp::Put {
                    key,
                    value,
                    options,
                } => {
                    self.set_tx(conn, key, value, options).await?;
                }
                BatchOp::Delete { key } => {
                    self.delete_tx(conn, &key).await?;
                }
            }
        }
        Ok(())
    }

    async fn ttl_tx(
        &self,
        conn: &mut sqlx::PgConnection,
        key: &Key,
    ) -> Result<Option<Duration>, KvError> {
        let row = sqlx::query(
            "SELECT EXTRACT(EPOCH FROM (expires_at - NOW())) as ttl_secs FROM kv_entries WHERE key = $1 AND (expires_at IS NULL OR expires_at > NOW())",
        )
        .bind(key.as_bytes())
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| KvError::StoreError(e.to_string()))?;

        if let Some(row) = row {
            let secs: Option<f64> = row.get("ttl_secs");
            if let Some(s) = secs.filter(|&s| s > 0.0) {
                return Ok(Some(Duration::from_secs_f64(s)));
            }
        }
        Ok(None)
    }

    async fn clear_tx(&self, conn: &mut sqlx::PgConnection) -> Result<(), KvError> {
        sqlx::query("TRUNCATE TABLE kv_entries")
            .execute(&mut *conn)
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        Ok(())
    }

    async fn clean_expired_tx(
        &self,
        conn: &mut sqlx::PgConnection,
        limit: Option<usize>,
    ) -> Result<u64, KvError> {
        let max_limit = limit.unwrap_or(1000) as i64;
        let res = sqlx::query(
            "DELETE FROM kv_entries WHERE key IN (SELECT key FROM kv_entries WHERE expires_at IS NOT NULL AND expires_at <= NOW() LIMIT $1)",
        )
        .bind(max_limit)
        .execute(&mut *conn)
        .await
        .map_err(|e| KvError::StoreError(e.to_string()))?;

        Ok(res.rows_affected())
    }
}

// ---------------------------------------------------------------------------
// KvStore — delegates to KvStoreTx by acquiring a connection from the pool
// ---------------------------------------------------------------------------

#[async_trait]
impl KvStore for PostgresKvStore {
    async fn get(&self, key: &Key) -> Result<Option<Value>, KvError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        <Self as KvStoreTx<sqlx::PgConnection>>::get_tx(self, &mut conn, key).await
    }

    async fn set(&self, key: Key, value: Value, options: SetOptions) -> Result<(), KvError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        <Self as KvStoreTx<sqlx::PgConnection>>::set_tx(self, &mut conn, key, value, options).await
    }

    async fn delete(&self, key: &Key) -> Result<bool, KvError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        <Self as KvStoreTx<sqlx::PgConnection>>::delete_tx(self, &mut conn, key).await
    }

    async fn exists(&self, key: &Key) -> Result<bool, KvError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        <Self as KvStoreTx<sqlx::PgConnection>>::exists_tx(self, &mut conn, key).await
    }

    async fn batch(&self, ops: Vec<BatchOp>) -> Result<(), KvError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;

        <Self as KvStoreTx<sqlx::PgConnection>>::batch_tx(self, &mut tx, ops).await?;

        tx.commit()
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        Ok(())
    }

    async fn scan(&self, options: ScanOptions) -> KvStream {
        let pool = self.pool.clone();

        let s = stream! {
            let mut sql = "SELECT key, value, expires_at FROM kv_entries WHERE (expires_at IS NULL OR expires_at > NOW())".to_string();

            if let Some(ref prefix) = options.prefix {
                sql.push_str(" AND key LIKE ");
                let escaped_prefix = format!("'{}%'", prefix.to_string().replace('\'', "''"));
                sql.push_str(&escaped_prefix);
            }

            if options.reverse {
                sql.push_str(" ORDER BY key DESC");
            } else {
                sql.push_str(" ORDER BY key ASC");
            }

            if let Some(limit) = options.limit {
                sql.push_str(&format!(" LIMIT {}", limit));
            }

            let rows = match sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).fetch_all(&pool).await {
                Ok(r) => r,
                Err(e) => {
                    yield Err(KvError::StoreError(e.to_string()));
                    return;
                }
            };

            for row in rows {
                let k_bytes: Vec<u8> = row.get("key");
                let v_bytes: Vec<u8> = row.get("value");
                yield Ok(KvEntry {
                    key: Key::new(k_bytes),
                    value: Value::new(v_bytes),
                    expires_at: None,
                });
            }
        };

        Box::pin(s)
    }

    async fn ttl(&self, key: &Key) -> Result<Option<Duration>, KvError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        <Self as KvStoreTx<sqlx::PgConnection>>::ttl_tx(self, &mut conn, key).await
    }

    async fn clear(&self) -> Result<(), KvError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        <Self as KvStoreTx<sqlx::PgConnection>>::clear_tx(self, &mut conn).await
    }

    async fn clean_expired(&self, limit: Option<usize>) -> Result<u64, KvError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        <Self as KvStoreTx<sqlx::PgConnection>>::clean_expired_tx(self, &mut conn, limit).await
    }
}
