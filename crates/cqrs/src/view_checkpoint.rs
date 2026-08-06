use async_trait::async_trait;
use event_sourcing::SequencePosition;
use kv_store::{Key, KvStore, SetOptions, Value};
use thiserror::Error;

/// Error type for CheckpointStore operations.
#[derive(Debug, Error)]
pub enum CheckpointError {
    #[error("Checkpoint store error: {0}")]
    Store(String),

    #[error("Parse error: {0}")]
    Parse(String),
}

/// Abstract storage interface for tracking projection progress positions for views.
#[async_trait]
pub trait CheckpointStore: Send + Sync {
    /// Retrieve the last processed position for a view table name.
    async fn get_position(
        &self,
        view_name: &str,
    ) -> Result<Option<SequencePosition>, CheckpointError>;

    /// Save the newly committed position for a view table name.
    async fn save_position(
        &self,
        view_name: &str,
        position: SequencePosition,
    ) -> Result<(), CheckpointError>;
}

/// Adapter allowing any [`KvStore`] from `kv-store` to be used as a [`CheckpointStore`].
pub struct KvCheckpointStore<K: KvStore> {
    store: K,
    key_prefix: String,
}

impl<K: KvStore> KvCheckpointStore<K> {
    pub fn new(store: K) -> Self {
        Self {
            store,
            key_prefix: "cqrs:checkpoint:".to_string(),
        }
    }

    pub fn with_prefix(store: K, key_prefix: impl Into<String>) -> Self {
        Self {
            store,
            key_prefix: key_prefix.into(),
        }
    }

    fn make_key(&self, view_name: &str) -> Key {
        Key::new(format!("{}{}", self.key_prefix, view_name))
    }
}

#[async_trait]
impl<K: KvStore> CheckpointStore for KvCheckpointStore<K> {
    async fn get_position(
        &self,
        view_name: &str,
    ) -> Result<Option<SequencePosition>, CheckpointError> {
        let key = self.make_key(view_name);
        match self
            .store
            .get(&key)
            .await
            .map_err(|e| CheckpointError::Store(e.to_string()))?
        {
            Some(value) => {
                let pos_str = value.as_str().ok_or_else(|| {
                    CheckpointError::Parse("Value is not a valid UTF-8 string".to_string())
                })?;
                let pos_val: u64 = pos_str
                    .parse()
                    .map_err(|e: std::num::ParseIntError| CheckpointError::Parse(e.to_string()))?;
                Ok(Some(SequencePosition::new(pos_val)))
            }
            None => Ok(None),
        }
    }

    async fn save_position(
        &self,
        view_name: &str,
        position: SequencePosition,
    ) -> Result<(), CheckpointError> {
        let key = self.make_key(view_name);
        let value = Value::from(position.value().to_string());
        self.store
            .set(key, value, SetOptions::default())
            .await
            .map_err(|e| CheckpointError::Store(e.to_string()))
    }
}
