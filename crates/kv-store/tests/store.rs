//! Integration test for the KvStore trait and MockKvStore.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use kv_store::{KvStore, MockKvStore, PutCondition, ReplacedValue, StoredValue};
use serde_json::json;

#[tokio::test]
async fn mock_kv_store_get_and_put() {
    let mut mock = MockKvStore::new();
    mock.expect_get()
        .with(mockall::predicate::eq("k"))
        .returning(|_| {
            Ok(Some(StoredValue {
                key: "k".to_string(),
                value: json!({"n": 1}),
                created_at: None,
                updated_at: None,
            }))
        });

    let result = mock.get("k").await.unwrap();
    assert_eq!(
        result,
        Some(StoredValue {
            key: "k".to_string(),
            value: json!({"n": 1}),
            created_at: None,
            updated_at: None,
        })
    );

    mock.expect_put()
        .with(
            mockall::predicate::eq("k"),
            mockall::predicate::eq(json!({"n": 2})),
            mockall::predicate::eq(Some(PutCondition::Absent)),
        )
        .returning(|k, v, _| {
            Ok(ReplacedValue {
                key: k.to_string(),
                value: v,
                previous: None,
                created_at: None,
                updated_at: None,
            })
        });

    let put_res = mock
        .put("k", json!({"n": 2}), Some(PutCondition::Absent))
        .await
        .unwrap();
    assert_eq!(put_res.value, json!({"n": 2}));
}

#[tokio::test]
async fn mock_kv_store_delete() {
    let mut mock = MockKvStore::new();
    mock.expect_delete()
        .with(mockall::predicate::eq("k"))
        .returning(|k| {
            Ok(Some(StoredValue {
                key: k.to_string(),
                value: json!({"n": 1}),
                created_at: None,
                updated_at: None,
            }))
        });

    let result = mock.delete("k").await.unwrap();
    assert_eq!(
        result,
        Some(StoredValue {
            key: "k".to_string(),
            value: json!({"n": 1}),
            created_at: None,
            updated_at: None,
        })
    );
}
