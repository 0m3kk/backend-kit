use std::collections::BTreeMap;
use std::ops::Bound;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use futures_util::stream;
use tokio::sync::RwLock;

use crate::errors::KvError;
use crate::store::{KvStore, KvStream};
use crate::types::{BatchOp, Key, KvEntry, ScanOptions, SetOptions, Value};

#[derive(Debug, Clone)]
struct MemoryEntry {
    value: Value,
    expires_at: Option<SystemTime>,
}

impl MemoryEntry {
    fn is_expired(&self) -> bool {
        if let Some(exp) = self.expires_at {
            SystemTime::now() >= exp
        } else {
            false
        }
    }

    fn remaining_ttl(&self) -> Option<Duration> {
        let exp = self.expires_at?;
        let now = SystemTime::now();
        if exp > now {
            exp.duration_since(now).ok()
        } else {
            None
        }
    }
}

/// Thread-safe in-memory Key-Value store implementing the `KvStore` trait.
#[derive(Debug, Clone, Default)]
pub struct MemoryKvStore {
    state: Arc<RwLock<BTreeMap<Key, MemoryEntry>>>,
}

impl MemoryKvStore {
    /// Creates a new, empty in-memory Key-Value store instance.
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }
}

#[async_trait]
impl KvStore for MemoryKvStore {
    async fn get(&self, key: &Key) -> Result<Option<Value>, KvError> {
        let mut map = self.state.write().await;
        if let Some(entry) = map.get(key) {
            if entry.is_expired() {
                map.remove(key);
                Ok(None)
            } else {
                Ok(Some(entry.value.clone()))
            }
        } else {
            Ok(None)
        }
    }

    async fn set(&self, key: Key, value: Value, options: SetOptions) -> Result<(), KvError> {
        let mut map = self.state.write().await;

        let exists = match map.get(&key) {
            Some(e) if e.is_expired() => {
                map.remove(&key);
                false
            }
            Some(_) => true,
            None => false,
        };

        if options.if_not_exists && exists {
            return Err(KvError::ConditionFailed);
        }

        if options.if_exists && !exists {
            return Err(KvError::ConditionFailed);
        }

        let expires_at = options.ttl.map(|ttl| SystemTime::now() + ttl);

        map.insert(key, MemoryEntry { value, expires_at });
        Ok(())
    }

    async fn delete(&self, key: &Key) -> Result<bool, KvError> {
        let mut map = self.state.write().await;
        if let Some(entry) = map.remove(key) {
            Ok(!entry.is_expired())
        } else {
            Ok(false)
        }
    }

    async fn exists(&self, key: &Key) -> Result<bool, KvError> {
        let mut map = self.state.write().await;
        if let Some(entry) = map.get(key) {
            if entry.is_expired() {
                map.remove(key);
                Ok(false)
            } else {
                Ok(true)
            }
        } else {
            Ok(false)
        }
    }

    async fn batch(&self, ops: Vec<BatchOp>) -> Result<(), KvError> {
        let mut map = self.state.write().await;

        // Perform validation check for conditions first
        for op in &ops {
            if let BatchOp::Put { key, options, .. } = op {
                let exists = match map.get(key) {
                    Some(e) if e.is_expired() => false,
                    Some(_) => true,
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

        // Apply mutations atomically
        for op in ops {
            match op {
                BatchOp::Put {
                    key,
                    value,
                    options,
                } => {
                    let expires_at = options.ttl.map(|ttl| SystemTime::now() + ttl);
                    map.insert(key, MemoryEntry { value, expires_at });
                }
                BatchOp::Delete { key } => {
                    map.remove(&key);
                }
            }
        }

        Ok(())
    }

    async fn scan(&self, options: ScanOptions) -> KvStream {
        let map = self.state.read().await;
        let mut results: Vec<KvEntry> = Vec::new();

        let start_bound = match &options.start {
            Some(k) => Bound::Included(k),
            None => Bound::Unbounded,
        };

        let end_bound = match &options.end {
            Some(k) => Bound::Included(k),
            None => Bound::Unbounded,
        };

        let now = SystemTime::now();

        let collect_entry = |(k, v): (&Key, &MemoryEntry)| -> Option<KvEntry> {
            if matches!(v.expires_at, Some(exp) if now >= exp) {
                return None;
            }

            if matches!(&options.prefix, Some(prefix) if !k.starts_with(prefix.as_bytes())) {
                return None;
            }

            Some(KvEntry {
                key: k.clone(),
                value: v.value.clone(),
                expires_at: v.expires_at,
            })
        };

        if options.reverse {
            for (k, v) in map.range((start_bound, end_bound)).rev() {
                if let Some(entry) = collect_entry((k, v)) {
                    results.push(entry);
                }
            }
        } else {
            for (k, v) in map.range((start_bound, end_bound)) {
                if let Some(entry) = collect_entry((k, v)) {
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
        let mut map = self.state.write().await;
        if let Some(entry) = map.get(key) {
            if entry.is_expired() {
                map.remove(key);
                Ok(None)
            } else {
                Ok(entry.remaining_ttl())
            }
        } else {
            Ok(None)
        }
    }

    async fn clear(&self) -> Result<(), KvError> {
        let mut map = self.state.write().await;
        map.clear();
        Ok(())
    }

    async fn clean_expired(&self, limit: Option<usize>) -> Result<u64, KvError> {
        let mut map = self.state.write().await;
        let max_remove = limit.unwrap_or(usize::MAX);
        let expired_keys: Vec<Key> = map
            .iter()
            .filter(|(_, entry)| entry.is_expired())
            .map(|(k, _)| k.clone())
            .take(max_remove)
            .collect();

        let count = expired_keys.len() as u64;
        for key in expired_keys {
            map.remove(&key);
        }

        Ok(count)
    }
}

#[async_trait]
impl<Conn: Send> super::store::KvStoreTx<Conn> for MemoryKvStore {
    async fn get_tx(&self, _conn: &mut Conn, key: &Key) -> Result<Option<Value>, KvError> {
        self.get(key).await
    }

    async fn set_tx(
        &self,
        _conn: &mut Conn,
        key: Key,
        value: Value,
        options: SetOptions,
    ) -> Result<(), KvError> {
        self.set(key, value, options).await
    }

    async fn delete_tx(&self, _conn: &mut Conn, key: &Key) -> Result<bool, KvError> {
        self.delete(key).await
    }

    async fn exists_tx(&self, _conn: &mut Conn, key: &Key) -> Result<bool, KvError> {
        self.exists(key).await
    }

    async fn batch_tx(&self, _conn: &mut Conn, ops: Vec<BatchOp>) -> Result<(), KvError> {
        self.batch(ops).await
    }

    async fn ttl_tx(&self, _conn: &mut Conn, key: &Key) -> Result<Option<Duration>, KvError> {
        self.ttl(key).await
    }

    async fn clear_tx(&self, _conn: &mut Conn) -> Result<(), KvError> {
        self.clear().await
    }

    async fn clean_expired_tx(
        &self,
        _conn: &mut Conn,
        limit: Option<usize>,
    ) -> Result<u64, KvError> {
        self.clean_expired(limit).await
    }
}


