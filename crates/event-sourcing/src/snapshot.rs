use async_trait::async_trait;
use futures_util::StreamExt;
use kv_store::{KvStore, SetOptions, Value};
use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;

use crate::decision::{DecisionModel, LoadedModel};
use crate::store::{EventStore, ReadError};
use crate::types::{Query, ReadOptions};

/// Hardcoded key prefix for Decision Model snapshots in KV store.
pub const SNAPSHOT_PREFIX: &str = "decision_model:snapshot";

/// Computes the deterministic KV store key for a Decision Model query.
pub fn snapshot_key(query: &Query) -> kv_store::Key {
    kv_store::Key::from(format!("{}:{}", SNAPSHOT_PREFIX, query.fingerprint()))
}

/// Errors that can occur during decision model snapshot loading or saving.
#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("Event store read error: {0}")]
    Read(#[from] ReadError),

    #[error("KV store error: {0}")]
    Kv(#[from] kv_store::KvError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Options controlling snapshot loading and auto-saving behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotOptions {
    /// Minimum number of newly applied events required to trigger an automatic snapshot write.
    ///
    /// - `0`: Always update snapshot if any new event was read.
    /// - `N > 0`: Only update snapshot if `new_events_count >= N`.
    pub threshold: usize,
}

impl SnapshotOptions {
    pub fn new(threshold: usize) -> Self {
        Self { threshold }
    }

    pub fn with_threshold(mut self, threshold: usize) -> Self {
        self.threshold = threshold;
        self
    }
}

impl Default for SnapshotOptions {
    fn default() -> Self {
        Self { threshold: 100 }
    }
}

impl<M: DecisionModel + Serialize + DeserializeOwned> LoadedModel<M> {
    /// Saves the current hydrated model state and sequence position to KV Store under the hardcoded snapshot key.
    pub async fn save_snapshot(&self, kv: &(impl KvStore + ?Sized)) -> Result<(), SnapshotError> {
        let key = snapshot_key(&self.model.query());
        let val = Value::from_json(self)?;
        kv.set(key, val, SetOptions::default()).await?;
        Ok(())
    }
}

/// Extension trait for [`EventStore`] adding snapshot-backed decision model loading.
#[async_trait]
pub trait EventStoreSnapshotExt: EventStore {
    /// Hydrates a decision model using a KV snapshot if available, catching up with new events from the store,
    /// and automatically updating the snapshot in KV store if newly applied events meet or exceed `options.threshold`.
    async fn load_decision_model_with_snapshot<M>(
        &self,
        kv: &(impl KvStore + ?Sized),
        model: M,
        options: SnapshotOptions,
    ) -> Result<LoadedModel<M>, SnapshotError>
    where
        M: DecisionModel + Serialize + DeserializeOwned,
    {
        let query = model.query();
        let key = snapshot_key(&query);

        // 1. Attempt to load snapshot from KV store
        let (mut loaded, _had_snapshot) = match kv.get(&key).await {
            Ok(Some(val)) => match val.to_json::<LoadedModel<M>>() {
                Ok(snap) => (snap, true),
                Err(_) => (LoadedModel::new(model), false),
            },
            _ => (LoadedModel::new(model), false),
        };

        // 2. Set read options to catch up starting after snapshot position
        let read_opts = match loaded.last_position {
            Some(pos) => ReadOptions::default().after(pos),
            None => ReadOptions::default(),
        };

        let mut stream = self.read(&query, read_opts).await;
        let mut new_events_count = 0;

        while let Some(res) = stream.next().await {
            let seq_event = res.map_err(SnapshotError::Read)?;
            loaded.apply_sequenced(&seq_event);
            new_events_count += 1;
        }

        // 3. Auto-save snapshot if newly processed events meet or exceed threshold
        let should_snapshot = if options.threshold == 0 {
            new_events_count > 0
        } else {
            new_events_count >= options.threshold
        };

        if should_snapshot {
            loaded.save_snapshot(kv).await?;
        }

        Ok(loaded)
    }
}

impl<T: EventStore + ?Sized> EventStoreSnapshotExt for T {}
