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
///
/// ### Storage Context (`C`)
/// Parameterized by storage context `C` (`CheckpointStore<C>`) to allow saving checkpoints
/// within the same database transaction context `ctx` (`&C`) as your view table mutations.
///
/// When using non-transactional global KV stores (such as Redis or Redb), `C` defaults to `()`.
#[async_trait]
pub trait CheckpointStore<C = ()>: Send + Sync {
    /// Retrieve the last processed position for a view table name using context `ctx` (`&C`).
    async fn get_position(
        &self,
        ctx: &C,
        view_name: &str,
    ) -> Result<Option<SequencePosition>, CheckpointError>;

    /// Save the newly committed position for a view table name using context `ctx` (`&C`).
    async fn save_position(
        &self,
        ctx: &C,
        view_name: &str,
        position: SequencePosition,
    ) -> Result<(), CheckpointError>;
}

/// Adapter allowing any [`KvStore`] from `kv-store` to be used as a [`CheckpointStore<C>`].
///
/// Ignores storage context `_ctx` and persists checkpoints using the underlying [`KvStore`].
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

    async fn get_internal(
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

    async fn save_internal(
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

#[async_trait]
impl<K: KvStore, C: Send + Sync> CheckpointStore<C> for KvCheckpointStore<K> {
    async fn get_position(
        &self,
        _ctx: &C,
        view_name: &str,
    ) -> Result<Option<SequencePosition>, CheckpointError> {
        self.get_internal(view_name).await
    }

    async fn save_position(
        &self,
        _ctx: &C,
        view_name: &str,
        position: SequencePosition,
    ) -> Result<(), CheckpointError> {
        self.save_internal(view_name, position).await
    }
}
