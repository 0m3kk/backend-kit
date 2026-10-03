use async_trait::async_trait;
use serde_json::Value;

use crate::condition::PutCondition;
use crate::error::Error;
use crate::value::{ReplacedValue, StoredValue};

/// A key-value store.
#[mockall::automock(type Error = std::io::Error;)]
#[async_trait]
pub trait KvStore: Send + Sync {
    /// Error type of the underlying store.
    type Error: Send + Sync + 'static;

    /// Read the value stored under the key, or `None` if the key does not
    /// exist.
    async fn get(&self, key: &str) -> Result<Option<StoredValue>, Error<Self::Error>>;

    /// Atomically store the value under the key.
    ///
    /// If a [PutCondition] is given and it is not met by the current value,
    /// the put fails with [`Error::Condition`] and the key is left unchanged.
    async fn put(
        &self,
        key: &str,
        value: Value,
        condition: Option<PutCondition>,
    ) -> Result<ReplacedValue, Error<Self::Error>>;

    /// Delete the key and return the value that was stored under it, or
    /// `None` if the key did not exist.
    async fn delete(&self, key: &str) -> Result<Option<StoredValue>, Error<Self::Error>>;
}
