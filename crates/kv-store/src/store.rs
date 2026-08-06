use async_trait::async_trait;
use futures_util::stream::BoxStream;
use std::time::Duration;

use crate::errors::KvError;
use crate::types::{BatchOp, Key, KvEntry, ScanOptions, SetOptions, Value};

/// Owned, pinned, thread-safe stream of Key-Value entries.
pub type KvStream = BoxStream<'static, Result<KvEntry, KvError>>;

/// Universal Key-Value Store interface providing async operations, scanning,
/// batch updates, and expiration semantics.
#[async_trait]
pub trait KvStore: Send + Sync {
    /// Retrieve a value by key. Returns `None` if the key does not exist or has expired.
    async fn get(&self, key: &Key) -> Result<Option<Value>, KvError>;

    /// Set a key to a value with optional parameters (TTL, `if_not_exists`, `if_exists`).
    async fn set(&self, key: Key, value: Value, options: SetOptions) -> Result<(), KvError>;

    /// Delete a key from the store. Returns `true` if key was present and removed.
    async fn delete(&self, key: &Key) -> Result<bool, KvError>;

    /// Check if a key exists and is not expired.
    async fn exists(&self, key: &Key) -> Result<bool, KvError>;

    /// Atomically execute a batch of `Put` and `Delete` operations.
    async fn batch(&self, ops: Vec<BatchOp>) -> Result<(), KvError>;

    /// Returns a stream of `KvEntry` matching the given `ScanOptions`.
    async fn scan(&self, options: ScanOptions) -> KvStream;

    /// Retrieve the remaining Time-To-Live (`Duration`) for a key if set and not expired.
    async fn ttl(&self, key: &Key) -> Result<Option<Duration>, KvError>;

    /// Remove all entries from the store.
    async fn clear(&self) -> Result<(), KvError>;

    /// Purge up to `limit` expired key-value entries from the store.
    /// Returns the number of entries removed.
    async fn clean_expired(&self, limit: Option<usize>) -> Result<u64, KvError>;
}
