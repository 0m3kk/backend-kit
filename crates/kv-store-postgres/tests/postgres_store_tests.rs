#![allow(clippy::unwrap_used, clippy::expect_used)]

use futures_util::StreamExt;
use kv_store::{BatchOp, Key, KvError, KvStore, ScanOptions, SetOptions, Value};
use kv_store_postgres::PostgresKvStore;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct OrderInfo {
    id: u64,
    item: String,
}

#[tokio::test]
async fn test_postgres_store_builder_options() {
    let mock_pool_res =
        PgPoolOptions::new().connect_lazy("postgres://postgres:postgres@localhost:5432/test_db");

    assert!(mock_pool_res.is_ok());
    let pool = mock_pool_res.unwrap();

    let store = PostgresKvStore::new(pool);
    assert!(store.get(&Key::from("dummy")).await.is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_postgres_kv_get_set_delete(pool: PgPool) {
    let store = PostgresKvStore::new(pool);

    let key = Key::from("user:1");
    let val = Value::from("Alice");

    assert_eq!(store.get(&key).await.unwrap(), None);
    assert!(!store.exists(&key).await.unwrap());

    store
        .set(key.clone(), val.clone(), SetOptions::new())
        .await
        .unwrap();

    assert_eq!(store.get(&key).await.unwrap(), Some(val));
    assert!(store.exists(&key).await.unwrap());

    assert!(store.delete(&key).await.unwrap());
    assert_eq!(store.get(&key).await.unwrap(), None);
    assert!(!store.delete(&key).await.unwrap());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_postgres_kv_nx_xx_conditions(pool: PgPool) {
    let store = PostgresKvStore::new(pool);

    let key = Key::from("cond_key");
    let val1 = Value::from("v1");
    let val2 = Value::from("v2");

    // set XX when key missing -> ConditionFailed
    assert_eq!(
        store
            .set(key.clone(), val1.clone(), SetOptions::new().if_exists())
            .await,
        Err(KvError::ConditionFailed)
    );

    // set NX when key missing -> Ok
    store
        .set(key.clone(), val1.clone(), SetOptions::new().if_not_exists())
        .await
        .unwrap();

    // set NX when key exists -> ConditionFailed
    assert_eq!(
        store
            .set(key.clone(), val2.clone(), SetOptions::new().if_not_exists())
            .await,
        Err(KvError::ConditionFailed)
    );

    // set XX when key exists -> Ok
    store
        .set(key.clone(), val2.clone(), SetOptions::new().if_exists())
        .await
        .unwrap();
    assert_eq!(store.get(&key).await.unwrap(), Some(val2));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_postgres_kv_batch_and_scan(pool: PgPool) {
    let store = PostgresKvStore::new(pool);

    let k1 = Key::from("user:10");
    let k2 = Key::from("user:20");
    let k3 = Key::from("order:1");

    let ops = vec![
        BatchOp::Put {
            key: k1.clone(),
            value: Value::from("val10"),
            options: SetOptions::new(),
        },
        BatchOp::Put {
            key: k2.clone(),
            value: Value::from("val20"),
            options: SetOptions::new(),
        },
        BatchOp::Put {
            key: k3.clone(),
            value: Value::from("ord1"),
            options: SetOptions::new(),
        },
    ];

    store.batch(ops).await.unwrap();

    let mut stream = store.scan(ScanOptions::new().with_prefix("user:")).await;
    let mut keys = Vec::new();
    while let Some(res) = stream.next().await {
        keys.push(res.unwrap().key.to_string());
    }

    assert_eq!(keys, vec!["user:10", "user:20"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_postgres_kv_batch_atomic_rollback(pool: PgPool) {
    let store = PostgresKvStore::new(pool);

    store
        .set(Key::from("existing"), Value::from("v1"), SetOptions::new())
        .await
        .unwrap();

    let failing_ops = vec![
        BatchOp::Put {
            key: Key::from("new_key"),
            value: Value::from("v2"),
            options: SetOptions::new(),
        },
        BatchOp::Put {
            key: Key::from("existing"),
            value: Value::from("v3"),
            options: SetOptions::new().if_not_exists(),
        },
    ];

    assert_eq!(
        store.batch(failing_ops).await,
        Err(KvError::ConditionFailed)
    );
    assert_eq!(store.get(&Key::from("new_key")).await.unwrap(), None);
    assert_eq!(
        store.get(&Key::from("existing")).await.unwrap(),
        Some(Value::from("v1"))
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_postgres_kv_json_and_binary(pool: PgPool) {
    let store = PostgresKvStore::new(pool);

    let order = OrderInfo {
        id: 99,
        item: "Rust Book".to_string(),
    };
    let key = Key::from("order:99");
    let val = Value::from_json(&order).unwrap();

    store
        .set(key.clone(), val, SetOptions::new())
        .await
        .unwrap();

    let restored: OrderInfo = store.get(&key).await.unwrap().unwrap().to_json().unwrap();
    assert_eq!(restored, order);

    // Large binary test
    let bin_key = Key::new(vec![0x00, 0x11, 0x22]);
    let bin_val = Value::new(vec![128u8; 500_000]); // 500KB
    store
        .set(bin_key.clone(), bin_val.clone(), SetOptions::new())
        .await
        .unwrap();

    assert_eq!(store.get(&bin_key).await.unwrap(), Some(bin_val));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_postgres_kv_clear(pool: PgPool) {
    let store = PostgresKvStore::new(pool);

    store
        .set(Key::from("k1"), Value::from("v1"), SetOptions::new())
        .await
        .unwrap();
    store
        .set(Key::from("k2"), Value::from("v2"), SetOptions::new())
        .await
        .unwrap();

    assert!(store.exists(&Key::from("k1")).await.unwrap());
    store.clear().await.unwrap();
    assert!(!store.exists(&Key::from("k1")).await.unwrap());
    assert!(!store.exists(&Key::from("k2")).await.unwrap());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_postgres_kv_concurrency(pool: PgPool) {
    let store = Arc::new(PostgresKvStore::new(pool));
    let mut handles = Vec::new();

    for i in 0..10 {
        let store_clone = Arc::clone(&store);
        let handle = tokio::spawn(async move {
            let key = Key::from(format!("pg:user:{i}"));
            let val = Value::from(format!("val:{i}"));
            store_clone
                .set(key.clone(), val.clone(), SetOptions::new())
                .await
                .unwrap();
            let retrieved = store_clone.get(&key).await.unwrap();
            assert_eq!(retrieved, Some(val));
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_postgres_kv_clean_expired(pool: PgPool) {
    let store = PostgresKvStore::new(pool);

    store
        .set(
            Key::from("k_exp"),
            Value::from("v1"),
            SetOptions::new().with_ttl(std::time::Duration::from_millis(1)),
        )
        .await
        .unwrap();
    store
        .set(Key::from("k_valid"), Value::from("v2"), SetOptions::new())
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let removed = store.clean_expired(Some(10)).await.unwrap();
    assert_eq!(removed, 1);
    assert!(!store.exists(&Key::from("k_exp")).await.unwrap());
    assert!(store.exists(&Key::from("k_valid")).await.unwrap());
}
