use std::ops::Bound;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use futures_util::stream;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};

use kv_store::{BatchOp, Key, KvEntry, KvError, KvStore, KvStream, ScanOptions, SetOptions, Value};

const KV_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("kv_entries");

fn system_time_to_millis(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn millis_to_system_time(millis: u64) -> Option<SystemTime> {
    if millis == 0 {
        None
    } else {
        Some(UNIX_EPOCH + Duration::from_millis(millis))
    }
}

fn encode_payload(value: &[u8], expires_at: Option<SystemTime>) -> Vec<u8> {
    let exp_millis = expires_at.map(system_time_to_millis).unwrap_or(0);
    let mut buf = Vec::with_capacity(8 + value.len());
    buf.extend_from_slice(&exp_millis.to_le_bytes());
    buf.extend_from_slice(value);
    buf
}

fn decode_raw_payload(data: &[u8]) -> Option<(Value, Option<SystemTime>)> {
    if data.len() < 8 {
        return None;
    }
    let (exp_bytes, val_bytes) = data.split_at(8);
    let exp_millis = u64::from_le_bytes(exp_bytes.try_into().ok()?);
    let expires_at = millis_to_system_time(exp_millis);
    Some((Value::new(val_bytes), expires_at))
}

fn decode_payload(data: &[u8]) -> Option<(Value, Option<SystemTime>)> {
    let (val, expires_at) = decode_raw_payload(data)?;

    if matches!(expires_at, Some(exp) if SystemTime::now() >= exp) {
        return None; // Expired
    }

    Some((val, expires_at))
}

/// Embedded persistent Key-Value Store backed by the popular `redb` ACID storage engine.
#[derive(Clone)]
pub struct RedbKvStore {
    db: Arc<Database>,
}

impl RedbKvStore {
    /// Opens or creates a `redb` embedded database file at the specified file path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, KvError> {
        let db = Database::create(path).map_err(|e| KvError::StoreError(e.to_string()))?;
        let store = Self { db: Arc::new(db) };
        store.init_table()?;
        Ok(store)
    }

    fn init_table(&self) -> Result<(), KvError> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        {
            let _table = write_txn
                .open_table(KV_TABLE)
                .map_err(|e| KvError::StoreError(e.to_string()))?;
        }
        write_txn
            .commit()
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        Ok(())
    }
}

#[async_trait]
impl KvStore for RedbKvStore {
    async fn get(&self, key: &Key) -> Result<Option<Value>, KvError> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        let table = read_txn
            .open_table(KV_TABLE)
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        if let Some(access) = table
            .get(key.as_bytes())
            .map_err(|e| KvError::StoreError(e.to_string()))?
        {
            let data = access.value();
            if let Some((val, _)) = decode_payload(data) {
                Ok(Some(val))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    async fn set(&self, key: Key, value: Value, options: SetOptions) -> Result<(), KvError> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        {
            let mut table = write_txn
                .open_table(KV_TABLE)
                .map_err(|e| KvError::StoreError(e.to_string()))?;

            let exists = match table
                .get(key.as_bytes())
                .map_err(|e| KvError::StoreError(e.to_string()))?
            {
                Some(access) => decode_payload(access.value()).is_some(),
                None => false,
            };

            if options.if_not_exists && exists {
                return Err(KvError::ConditionFailed);
            }
            if options.if_exists && !exists {
                return Err(KvError::ConditionFailed);
            }

            let expires_at = options.ttl.map(|ttl| SystemTime::now() + ttl);
            let payload = encode_payload(value.as_bytes(), expires_at);
            table
                .insert(key.as_bytes(), payload.as_slice())
                .map_err(|e| KvError::StoreError(e.to_string()))?;
        }
        write_txn
            .commit()
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        Ok(())
    }

    async fn delete(&self, key: &Key) -> Result<bool, KvError> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        let existed = {
            let mut table = write_txn
                .open_table(KV_TABLE)
                .map_err(|e| KvError::StoreError(e.to_string()))?;
            let removed = table
                .remove(key.as_bytes())
                .map_err(|e| KvError::StoreError(e.to_string()))?;
            if let Some(access) = removed {
                decode_payload(access.value()).is_some()
            } else {
                false
            }
        };
        write_txn
            .commit()
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        Ok(existed)
    }

    async fn exists(&self, key: &Key) -> Result<bool, KvError> {
        Ok(self.get(key).await?.is_some())
    }

    async fn batch(&self, ops: Vec<BatchOp>) -> Result<(), KvError> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        {
            let mut table = write_txn
                .open_table(KV_TABLE)
                .map_err(|e| KvError::StoreError(e.to_string()))?;

            // Pre-condition check
            for op in &ops {
                if let BatchOp::Put { key, options, .. } = op {
                    let exists = match table
                        .get(key.as_bytes())
                        .map_err(|e| KvError::StoreError(e.to_string()))?
                    {
                        Some(access) => decode_payload(access.value()).is_some(),
                        None => false,
                    };
                    if options.if_not_exists && exists {
                        return Err(KvError::ConditionFailed);
                    }
                    if options.if_exists && !exists {
                        return Err(KvError::ConditionFailed);
                    }
                }
            }

            for op in ops {
                match op {
                    BatchOp::Put {
                        key,
                        value,
                        options,
                    } => {
                        let expires_at = options.ttl.map(|ttl| SystemTime::now() + ttl);
                        let payload = encode_payload(value.as_bytes(), expires_at);
                        table
                            .insert(key.as_bytes(), payload.as_slice())
                            .map_err(|e| KvError::StoreError(e.to_string()))?;
                    }
                    BatchOp::Delete { key } => {
                        table
                            .remove(key.as_bytes())
                            .map_err(|e| KvError::StoreError(e.to_string()))?;
                    }
                }
            }
        }
        write_txn
            .commit()
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        Ok(())
    }

    async fn scan(&self, options: ScanOptions) -> KvStream {
        let read_txn = match self.db.begin_read() {
            Ok(t) => t,
            Err(e) => return Box::pin(stream::iter(vec![Err(KvError::StoreError(e.to_string()))])),
        };

        let table = match read_txn.open_table(KV_TABLE) {
            Ok(t) => t,
            Err(e) => return Box::pin(stream::iter(vec![Err(KvError::StoreError(e.to_string()))])),
        };

        let start_bound = match &options.start {
            Some(k) => Bound::Included(k.as_bytes()),
            None => Bound::Unbounded,
        };

        let end_bound = match &options.end {
            Some(k) => Bound::Included(k.as_bytes()),
            None => Bound::Unbounded,
        };

        let range_iter = match table.range::<&[u8]>((start_bound, end_bound)) {
            Ok(iter) => iter,
            Err(e) => return Box::pin(stream::iter(vec![Err(KvError::StoreError(e.to_string()))])),
        };

        let mut results = Vec::new();

        let process_item = |k_bytes: &[u8], val_bytes: &[u8]| -> Option<KvEntry> {
            let (val, exp) = decode_payload(val_bytes)?;

            let key = Key::new(k_bytes);
            if matches!(&options.prefix, Some(prefix) if !key.starts_with(prefix.as_bytes())) {
                return None;
            }

            Some(KvEntry {
                key,
                value: val,
                expires_at: exp,
            })
        };

        if options.reverse {
            for (k, v) in range_iter.rev().flatten() {
                if let Some(entry) = process_item(k.value(), v.value()) {
                    results.push(entry);
                }
            }
        } else {
            for (k, v) in range_iter.flatten() {
                if let Some(entry) = process_item(k.value(), v.value()) {
                    results.push(entry);
                }
            }
        }

        if let Some(limit) = options.limit {
            results.truncate(limit);
        }

        Box::pin(stream::iter(results.into_iter().map(Ok)))
    }

    async fn ttl(&self, key: &Key) -> Result<Option<Duration>, KvError> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        let table = read_txn
            .open_table(KV_TABLE)
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        if let Some((_, Some(exp))) = table
            .get(key.as_bytes())
            .map_err(|e| KvError::StoreError(e.to_string()))?
            .and_then(|access| decode_payload(access.value()))
        {
            let now = SystemTime::now();
            if exp > now {
                return Ok(exp.duration_since(now).ok());
            }
        }
        Ok(None)
    }

    async fn clear(&self) -> Result<(), KvError> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        {
            let mut table = write_txn
                .open_table(KV_TABLE)
                .map_err(|e| KvError::StoreError(e.to_string()))?;
            let keys: Vec<Vec<u8>> = table
                .iter()
                .map_err(|e| KvError::StoreError(e.to_string()))?
                .filter_map(|item| item.ok().map(|(k, _)| k.value().to_vec()))
                .collect();
            for k in keys {
                table
                    .remove(k.as_slice())
                    .map_err(|e| KvError::StoreError(e.to_string()))?;
            }
        }
        write_txn
            .commit()
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        Ok(())
    }

    async fn clean_expired(&self, limit: Option<usize>) -> Result<u64, KvError> {
        let max_remove = limit.unwrap_or(usize::MAX);
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        let table = read_txn
            .open_table(KV_TABLE)
            .map_err(|e| KvError::StoreError(e.to_string()))?;

        let now = SystemTime::now();
        let mut expired_keys = Vec::new();

        for item in table
            .iter()
            .map_err(|e| KvError::StoreError(e.to_string()))?
        {
            let (k, v) = item.map_err(|e| KvError::StoreError(e.to_string()))?;
            if let Some((_, Some(exp))) = decode_raw_payload(v.value())
                && exp <= now
            {
                expired_keys.push(k.value().to_vec());
                if expired_keys.len() >= max_remove {
                    break;
                }
            }
        }
        drop(table);
        drop(read_txn);

        if expired_keys.is_empty() {
            return Ok(0);
        }

        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| KvError::StoreError(e.to_string()))?;
        {
            let mut table = write_txn
                .open_table(KV_TABLE)
                .map_err(|e| KvError::StoreError(e.to_string()))?;
            for k in &expired_keys {
                let _ = table.remove(k.as_slice());
            }
        }
        write_txn
            .commit()
            .map_err(|e| KvError::StoreError(e.to_string()))?;

        Ok(expired_keys.len() as u64)
    }
}
