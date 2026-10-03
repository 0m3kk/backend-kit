use async_trait::async_trait;

use crate::Snapshot;

/// A store for model snapshots, keyed by string.
#[mockall::automock(type Error = std::io::Error;)]
#[async_trait]
pub trait SnapshotStore: Send + Sync {
    type Error: Send + Sync + 'static;

    /// Read the snapshot stored under the key, or `None` if not found.
    async fn get(&self, key: &str) -> Result<Option<Snapshot>, Self::Error>;

    /// Persist the snapshot under the key, overwriting any previous snapshot.
    async fn put(&self, key: &str, snapshot: Snapshot) -> Result<(), Self::Error>;

    /// Check whether a snapshot should be written given the number of replayed events.
    fn should_snapshot(&self, _replayed_events: u64) -> bool {
        true
    }
}
