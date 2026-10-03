//! DCB compliance suite for the postgres snapshot store, run against a real
//! database via `#[sqlx::test]`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;
use snapshot_store::{PostgresSnapshotStore, Snapshot, SnapshotStore};
use sqlx::PgPool;

#[sqlx::test]
async fn get_missing_returns_none(pool: PgPool) {
    let store = PostgresSnapshotStore::new(pool);
    let result = store.get("missing").await.unwrap();
    assert!(result.is_none());
}

#[sqlx::test]
async fn put_then_get_roundtrips(pool: PgPool) {
    let store = PostgresSnapshotStore::new(pool);
    let snapshot = Snapshot::new(Some(42), json!({"balance": 100}));

    store.put("account:1", snapshot.clone()).await.unwrap();

    let result = store.get("account:1").await.unwrap();
    assert_eq!(result, Some(snapshot));
}

#[sqlx::test]
async fn put_with_none_position(pool: PgPool) {
    let store = PostgresSnapshotStore::new(pool);
    let snapshot = Snapshot::new(None, json!({"balance": 0}));

    store.put("account:2", snapshot.clone()).await.unwrap();

    let result = store.get("account:2").await.unwrap();
    assert_eq!(result, Some(snapshot));
}

#[sqlx::test]
async fn put_overwrites_existing_snapshot(pool: PgPool) {
    let store = PostgresSnapshotStore::new(pool);

    store
        .put("account:1", Snapshot::new(Some(10), json!({"balance": 50})))
        .await
        .unwrap();

    store
        .put(
            "account:1",
            Snapshot::new(Some(20), json!({"balance": 150})),
        )
        .await
        .unwrap();

    let result = store.get("account:1").await.unwrap();
    assert_eq!(
        result,
        Some(Snapshot::new(Some(20), json!({"balance": 150})))
    );
}

#[sqlx::test]
async fn threshold_evaluation(pool: PgPool) {
    let store_without_threshold = PostgresSnapshotStore::new(pool.clone());
    assert!(store_without_threshold.should_snapshot(0));
    assert!(store_without_threshold.should_snapshot(10));

    let store_with_threshold = PostgresSnapshotStore::new(pool).with_threshold(100);
    assert_eq!(store_with_threshold.threshold(), Some(100));
    assert!(!store_with_threshold.should_snapshot(99));
    assert!(store_with_threshold.should_snapshot(100));
    assert!(store_with_threshold.should_snapshot(101));
}
