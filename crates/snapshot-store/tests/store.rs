//! Integration tests for SnapshotStore trait and MockSnapshotStore.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;
use snapshot_store::{MockSnapshotStore, Snapshot, SnapshotStore};

#[tokio::test]
async fn mock_snapshot_store_get_and_put() {
    let mut mock = MockSnapshotStore::new();
    mock.expect_get()
        .with(mockall::predicate::eq("model:1"))
        .returning(|_| Ok(Some(Snapshot::new(Some(5), json!({"count": 10})))));

    let result = mock.get("model:1").await.unwrap();
    assert_eq!(result, Some(Snapshot::new(Some(5), json!({"count": 10}))));
}
