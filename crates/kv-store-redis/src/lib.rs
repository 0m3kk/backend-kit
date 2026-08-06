use std::time::Duration;

use async_stream::stream;
use async_trait::async_trait;
use redis::AsyncCommands;
use redis::aio::ConnectionManager;

use kv_store::{BatchOp, Key, KvEntry, KvError, KvStore, KvStream, ScanOptions, SetOptions, Value};

/// Redis backed Key-Value Store implementing the `KvStore` trait using `redis-rs`.
#[derive(Clone)]
pub struct RedisKvStore {
    conn: ConnectionManager,
}

impl RedisKvStore {
    /// Creates a new `RedisKvStore` using a `redis::Client`.
    pub async fn new(client: redis::Client) -> Result<Self, KvError> {
        let conn = client
            .get_connection_manager()
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;

        Ok(Self { conn })
    }

    /// Constructs a `RedisKvStore` wrapping an existing `ConnectionManager`.
    pub fn from_connection_manager(conn: ConnectionManager) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl KvStore for RedisKvStore {
    async fn get(&self, key: &Key) -> Result<Option<Value>, KvError> {
        let mut conn = self.conn.clone();
        let res: Option<Vec<u8>> = conn
            .get(key.as_bytes())
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;

        Ok(res.map(Value::new))
    }

    async fn set(&self, key: Key, value: Value, options: SetOptions) -> Result<(), KvError> {
        let mut conn = self.conn.clone();
        let mut cmd = redis::cmd("SET");
        cmd.arg(key.as_bytes()).arg(value.as_bytes());

        if options.if_not_exists {
            cmd.arg("NX");
        } else if options.if_exists {
            cmd.arg("XX");
        }

        if let Some(ttl) = options.ttl {
            let ms = u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX).max(1);
            cmd.arg("PX").arg(ms);
        }

        let res: Option<String> = cmd
            .query_async(&mut conn)
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;

        if (options.if_not_exists || options.if_exists) && res.is_none() {
            return Err(KvError::ConditionFailed);
        }

        Ok(())
    }

    async fn delete(&self, key: &Key) -> Result<bool, KvError> {
        let mut conn = self.conn.clone();
        let count: usize = conn
            .del(key.as_bytes())
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;

        Ok(count > 0)
    }

    async fn exists(&self, key: &Key) -> Result<bool, KvError> {
        let mut conn = self.conn.clone();
        let exists: bool = conn
            .exists(key.as_bytes())
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;

        Ok(exists)
    }

    async fn batch(&self, ops: Vec<BatchOp>) -> Result<(), KvError> {
        let mut conn = self.conn.clone();
        let mut pipe = redis::pipe();
        pipe.atomic();

        for op in ops {
            match op {
                BatchOp::Put {
                    key,
                    value,
                    options,
                } => {
                    let mut cmd = redis::cmd("SET");
                    cmd.arg(key.as_bytes()).arg(value.as_bytes());
                    if options.if_not_exists {
                        cmd.arg("NX");
                    } else if options.if_exists {
                        cmd.arg("XX");
                    }
                    if let Some(ttl) = options.ttl {
                        let ms = u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX).max(1);
                        cmd.arg("PX").arg(ms);
                    }
                    pipe.add_command(cmd);
                }
                BatchOp::Delete { key } => {
                    pipe.del(key.as_bytes());
                }
            }
        }

        let _res: Vec<redis::Value> = pipe
            .query_async(&mut conn)
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;

        Ok(())
    }

    async fn scan(&self, options: ScanOptions) -> KvStream {
        let mut conn = self.conn.clone();

        let s = stream! {
            let pattern = if let Some(ref prefix) = options.prefix {
                format!("{prefix}*")
            } else {
                "*".to_string()
            };

            let mut cursor: u64 = 0;

            loop {
                let mut iter_cmd = redis::cmd("SCAN");
                iter_cmd.arg(cursor).arg("MATCH").arg(&pattern);
                if let Some(limit) = options.limit {
                    iter_cmd.arg("COUNT").arg(limit);
                }

                let (next_cursor, keys): (u64, Vec<Vec<u8>>) = match iter_cmd.query_async(&mut conn).await {
                    Ok(res) => res,
                    Err(e) => {
                        yield Err(KvError::StoreError(e.to_string()));
                        return;
                    }
                };

                for key_bytes in keys {
                    let val_res: Result<Option<Vec<u8>>, redis::RedisError> = conn.get(&key_bytes).await;
                    match val_res {
                        Ok(Some(val_bytes)) => {
                            yield Ok(KvEntry {
                                key: Key::new(key_bytes),
                                value: Value::new(val_bytes),
                                expires_at: None,
                            });
                        }
                        Ok(None) => {}
                        Err(e) => {
                            yield Err(KvError::StoreError(e.to_string()));
                            return;
                        }
                    }
                }

                cursor = next_cursor;
                if cursor == 0 {
                    break;
                }
            }
        };

        Box::pin(s)
    }

    async fn ttl(&self, key: &Key) -> Result<Option<Duration>, KvError> {
        let mut conn = self.conn.clone();
        let pttl_ms: i64 = conn
            .pttl(key.as_bytes())
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;

        if pttl_ms > 0 {
            Ok(Some(Duration::from_millis(
                u64::try_from(pttl_ms).unwrap_or(0),
            )))
        } else {
            Ok(None)
        }
    }

    async fn clear(&self) -> Result<(), KvError> {
        let mut conn = self.conn.clone();
        let _: () = redis::cmd("FLUSHDB")
            .query_async(&mut conn)
            .await
            .map_err(|e| KvError::StoreError(e.to_string()))?;

        Ok(())
    }

    async fn clean_expired(&self, _limit: Option<usize>) -> Result<u64, KvError> {
        // Redis natively purges expired keys in the background via active sampling & passive eviction.
        Ok(0)
    }
}
