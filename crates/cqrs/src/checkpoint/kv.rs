use async_trait::async_trait;
use event_sourcing::SequencePosition;
use kv_store::{Key, KvStore, KvStoreTx, SetOptions, Value};

use super::{CheckpointError, CheckpointStore, CheckpointStoreTx};

/// Adapter allowing any [`KvStore`] from `kv-store` to be used as a [`CheckpointStore`].
#[derive(Debug, Clone)]
pub struct KvCheckpointStore<K> {
    store: K,
    key_prefix: String,
}

impl<K> KvCheckpointStore<K> {
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

    pub fn make_key(&self, view_name: &str) -> Key {
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

#[async_trait]
impl<K: KvStoreTx<Conn>, Conn: Send> CheckpointStoreTx<Conn> for KvCheckpointStore<K> {
    async fn get_position_tx(
        &self,
        conn: &mut Conn,
        view_name: &str,
    ) -> Result<Option<SequencePosition>, CheckpointError> {
        let key = self.make_key(view_name);
        match self
            .store
            .get_tx(conn, &key)
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

    async fn save_position_tx(
        &self,
        conn: &mut Conn,
        view_name: &str,
        position: SequencePosition,
    ) -> Result<(), CheckpointError> {
        let key = self.make_key(view_name);
        let value = Value::from(position.value().to_string());
        self.store
            .set_tx(conn, key, value, SetOptions::default())
            .await
            .map_err(|e| CheckpointError::Store(e.to_string()))
    }
}

/// Type alias for transactional KV checkpoint stores.
pub type KvCheckpointStoreTx<K> = KvCheckpointStore<K>;
