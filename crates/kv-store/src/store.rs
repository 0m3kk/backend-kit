use async_trait::async_trait;
use futures_util::stream::BoxStream;
use std::sync::Arc;
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

#[async_trait]
impl<T: KvStore + ?Sized> KvStore for Arc<T> {
    async fn get(&self, key: &Key) -> Result<Option<Value>, KvError> {
        (**self).get(key).await
    }

    async fn set(&self, key: Key, value: Value, options: SetOptions) -> Result<(), KvError> {
        (**self).set(key, value, options).await
    }

    async fn delete(&self, key: &Key) -> Result<bool, KvError> {
        (**self).delete(key).await
    }

    async fn exists(&self, key: &Key) -> Result<bool, KvError> {
        (**self).exists(key).await
    }

    async fn batch(&self, ops: Vec<BatchOp>) -> Result<(), KvError> {
        (**self).batch(ops).await
    }

    async fn scan(&self, options: ScanOptions) -> KvStream {
        (**self).scan(options).await
    }

    async fn ttl(&self, key: &Key) -> Result<Option<Duration>, KvError> {
        (**self).ttl(key).await
    }

    async fn clear(&self) -> Result<(), KvError> {
        (**self).clear().await
    }

    async fn clean_expired(&self, limit: Option<usize>) -> Result<u64, KvError> {
        (**self).clean_expired(limit).await
    }
}

/// KV store operations that can participate in an external transaction/connection.
///
/// `Conn` represents the connection or transaction handle type (e.g., `sqlx::PgConnection`).
/// The caller owns the connection/transaction lifecycle — the store only executes operations
/// through the provided handle.
#[async_trait]
pub trait KvStoreTx<Conn: Send>: Send + Sync {
    /// Retrieve a value by key using the provided connection handle.
    async fn get_tx(&self, conn: &mut Conn, key: &Key) -> Result<Option<Value>, KvError>;

    /// Set a key to a value with optional parameters using the provided connection handle.
    async fn set_tx(
        &self,
        conn: &mut Conn,
        key: Key,
        value: Value,
        options: SetOptions,
    ) -> Result<(), KvError>;

    /// Delete a key from the store using the provided connection handle.
    async fn delete_tx(&self, conn: &mut Conn, key: &Key) -> Result<bool, KvError>;

    /// Check if a key exists using the provided connection handle.
    async fn exists_tx(&self, conn: &mut Conn, key: &Key) -> Result<bool, KvError>;

    /// Atomically execute a batch of operations using the provided connection handle.
    async fn batch_tx(&self, conn: &mut Conn, ops: Vec<BatchOp>) -> Result<(), KvError>;

    /// Retrieve the remaining TTL for a key using the provided connection handle.
    async fn ttl_tx(&self, conn: &mut Conn, key: &Key) -> Result<Option<Duration>, KvError>;

    /// Remove all entries from the store using the provided connection handle.
    async fn clear_tx(&self, conn: &mut Conn) -> Result<(), KvError>;

    /// Purge up to `limit` expired entries using the provided connection handle.
    async fn clean_expired_tx(&self, conn: &mut Conn, limit: Option<usize>)
    -> Result<u64, KvError>;
}

#[async_trait]
impl<T: KvStoreTx<Conn> + ?Sized, Conn: Send> KvStoreTx<Conn> for Arc<T> {
    async fn get_tx(&self, conn: &mut Conn, key: &Key) -> Result<Option<Value>, KvError> {
        (**self).get_tx(conn, key).await
    }

    async fn set_tx(
        &self,
        conn: &mut Conn,
        key: Key,
        value: Value,
        options: SetOptions,
    ) -> Result<(), KvError> {
        (**self).set_tx(conn, key, value, options).await
    }

    async fn delete_tx(&self, conn: &mut Conn, key: &Key) -> Result<bool, KvError> {
        (**self).delete_tx(conn, key).await
    }

    async fn exists_tx(&self, conn: &mut Conn, key: &Key) -> Result<bool, KvError> {
        (**self).exists_tx(conn, key).await
    }

    async fn batch_tx(&self, conn: &mut Conn, ops: Vec<BatchOp>) -> Result<(), KvError> {
        (**self).batch_tx(conn, ops).await
    }

    async fn ttl_tx(&self, conn: &mut Conn, key: &Key) -> Result<Option<Duration>, KvError> {
        (**self).ttl_tx(conn, key).await
    }

    async fn clear_tx(&self, conn: &mut Conn) -> Result<(), KvError> {
        (**self).clear_tx(conn).await
    }

    async fn clean_expired_tx(
        &self,
        conn: &mut Conn,
        limit: Option<usize>,
    ) -> Result<u64, KvError> {
        (**self).clean_expired_tx(conn, limit).await
    }
}
