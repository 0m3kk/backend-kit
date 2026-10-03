//! Behavior suite for the postgres key-value store, run against a real
//! database via `#[sqlx::test]`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use kv_store::{Error, KvStore, PostgresKvStore, PutCondition};
use sqlx::PgPool;

async fn get_value(store: &PostgresKvStore, key: &str) -> Option<serde_json::Value> {
    store.get(key).await.unwrap().map(|v| v.value)
}

#[sqlx::test]
async fn get_missing_key_returns_none(pool: PgPool) {
    let store = PostgresKvStore::new(pool);
    let result = store.get("missing").await.unwrap();
    assert!(result.is_none());
}

#[sqlx::test]
async fn put_then_get_roundtrips(pool: PgPool) {
    let store = PostgresKvStore::new(pool);
    store
        .put("k", serde_json::json!({"n": 1}), None)
        .await
        .unwrap();

    let result = store.get("k").await.unwrap().unwrap();
    assert_eq!(result.key, "k");
    assert_eq!(result.value, serde_json::json!({"n": 1}));
}

#[sqlx::test]
async fn put_overwrites_and_reports_previous_value(pool: PgPool) {
    let store = PostgresKvStore::new(pool);
    store
        .put("k", serde_json::json!({"n": 1}), None)
        .await
        .unwrap();

    let result = store
        .put("k", serde_json::json!({"n": 2}), None)
        .await
        .unwrap();

    assert_eq!(result.key, "k");
    assert_eq!(result.value, serde_json::json!({"n": 2}));
    assert_eq!(result.previous, Some(serde_json::json!({"n": 1})));
}

#[sqlx::test]
async fn put_absent_creates_key_once(pool: PgPool) {
    let store = PostgresKvStore::new(pool);
    store
        .put("k", serde_json::json!({"n": 1}), Some(PutCondition::Absent))
        .await
        .unwrap();

    let result = store
        .put("k", serde_json::json!({"n": 2}), Some(PutCondition::Absent))
        .await;

    match result {
        Err(Error::Condition(error)) => {
            assert_eq!(error.current, Some(serde_json::json!({"n": 1})));
        }
        other => panic!("expected condition error, got {other:?}"),
    }

    assert_eq!(
        get_value(&store, "k").await,
        Some(serde_json::json!({"n": 1}))
    );
}

#[sqlx::test]
async fn put_value_is_compare_and_swap(pool: PgPool) {
    let store = PostgresKvStore::new(pool);
    store
        .put("k", serde_json::json!({"n": 1}), None)
        .await
        .unwrap();

    let result = store
        .put(
            "k",
            serde_json::json!({"n": 2}),
            Some(PutCondition::Value(serde_json::json!({"n": 1}))),
        )
        .await
        .unwrap();
    assert_eq!(result.previous, Some(serde_json::json!({"n": 1})));

    let result = store
        .put(
            "k",
            serde_json::json!({"n": 3}),
            Some(PutCondition::Value(serde_json::json!({"n": 99}))),
        )
        .await;
    assert!(matches!(result, Err(Error::Condition(_))));

    assert_eq!(
        get_value(&store, "k").await,
        Some(serde_json::json!({"n": 2}))
    );
}

#[sqlx::test]
async fn put_value_fails_when_key_missing(pool: PgPool) {
    let store = PostgresKvStore::new(pool);

    let result = store
        .put(
            "k",
            serde_json::json!({"n": 1}),
            Some(PutCondition::Value(serde_json::json!({"n": 0}))),
        )
        .await;

    match result {
        Err(Error::Condition(error)) => assert!(error.current.is_none()),
        other => panic!("expected condition error, got {other:?}"),
    }
}

#[sqlx::test]
async fn delete_removes_key_and_returns_value(pool: PgPool) {
    let store = PostgresKvStore::new(pool);
    store
        .put("k", serde_json::json!({"n": 1}), None)
        .await
        .unwrap();

    let result = store.delete("k").await.unwrap().unwrap();
    assert_eq!(result.value, serde_json::json!({"n": 1}));

    let result = store.delete("k").await.unwrap();
    assert!(result.is_none());
}

#[sqlx::test]
async fn keys_are_independent(pool: PgPool) {
    let store = PostgresKvStore::new(pool);
    store
        .put("a", serde_json::json!({"n": 1}), None)
        .await
        .unwrap();
    store
        .put("b", serde_json::json!({"n": 2}), None)
        .await
        .unwrap();

    store.delete("a").await.unwrap();

    assert!(store.get("a").await.unwrap().is_none());
    assert_eq!(
        get_value(&store, "b").await,
        Some(serde_json::json!({"n": 2}))
    );
}

#[sqlx::test]
async fn get_reports_created_at_and_updated_at(pool: PgPool) {
    let store = PostgresKvStore::new(pool);
    store
        .put("k", serde_json::json!({"n": 1}), None)
        .await
        .unwrap();

    let result = store.get("k").await.unwrap().unwrap();

    assert!(result.created_at.is_some());
    assert!(result.updated_at.is_some());
}

#[sqlx::test]
async fn put_preserves_created_at_and_bumps_updated_at(pool: PgPool) {
    let store = PostgresKvStore::new(pool);
    store
        .put("k", serde_json::json!({"n": 1}), None)
        .await
        .unwrap();
    let first = store.get("k").await.unwrap().unwrap();
    let created_at = first.created_at.unwrap();

    // Wait for the clock to advance past the previous statement.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    let result = store
        .put("k", serde_json::json!({"n": 2}), None)
        .await
        .unwrap();

    assert_eq!(result.created_at, Some(created_at));
    assert!(result.updated_at.unwrap() > created_at);

    let stored = store.get("k").await.unwrap().unwrap();
    assert_eq!(stored.created_at, Some(created_at));
    assert!(stored.updated_at.unwrap() > created_at);
}

#[sqlx::test]
async fn delete_returns_timestamps(pool: PgPool) {
    let store = PostgresKvStore::new(pool);
    store
        .put("k", serde_json::json!({"n": 1}), None)
        .await
        .unwrap();

    let result = store.delete("k").await.unwrap().unwrap();

    assert!(result.created_at.is_some());
    assert!(result.updated_at.is_some());
}

#[sqlx::test]
async fn stores_default_timestamps_for_rows_inserted_outside_the_store(pool: PgPool) {
    let store = PostgresKvStore::new(pool.clone());

    sqlx::query("INSERT INTO kv (key, value) VALUES ('raw', '{\"n\": 0}'::jsonb)")
        .execute(&pool)
        .await
        .unwrap();

    let result = store.get("raw").await.unwrap().unwrap();
    assert_eq!(result.key, "raw");
    assert_eq!(result.value, serde_json::json!({"n": 0}));
    assert!(result.created_at.is_some());
    assert!(result.updated_at.is_some());
}
