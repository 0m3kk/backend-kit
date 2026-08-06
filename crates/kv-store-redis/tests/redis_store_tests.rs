#![allow(clippy::unwrap_used, clippy::expect_used)]

use futures_util::StreamExt;
use kv_store::{BatchOp, Key, KvError, KvStore, ScanOptions, SetOptions, Value};
use kv_store_redis::RedisKvStore;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct UserProfile {
    id: u64,
    username: String,
}

#[tokio::test]
async fn test_redis_store_creation() -> Result<(), KvError> {
    let client_res = redis::Client::open("redis://127.0.0.1:6379");
    assert!(client_res.is_ok());
    Ok(())
}

async fn get_redis_store() -> RedisKvStore {
    let redis_url = match env::var("REDIS_URL") {
        Ok(url) => url,
        Err(_) => panic!(
            "REDIS_URL environment variable must be set to run Redis kv-store integration tests"
        ),
    };
    let client = match redis::Client::open(redis_url) {
        Ok(c) => c,
        Err(e) => panic!("Failed to create Redis client: {e}"),
    };
    match RedisKvStore::new(client).await {
        Ok(s) => s,
        Err(e) => panic!("Failed to connect to Redis server: {e}"),
    }
}

#[tokio::test]
async fn test_redis_kv_get_set_delete() -> Result<(), KvError> {
    let store = get_redis_store().await;

    let key = Key::from("test:getset:k1");
    let val = Value::from("v1");

    store.delete(&key).await?;

    assert_eq!(store.get(&key).await?, None);
    assert!(!store.exists(&key).await?);

    store
        .set(key.clone(), val.clone(), SetOptions::new())
        .await?;

    assert_eq!(store.get(&key).await?, Some(val));
    assert!(store.exists(&key).await?);

    assert!(store.delete(&key).await?);
    assert_eq!(store.get(&key).await?, None);
    assert!(!store.delete(&key).await?);

    Ok(())
}

#[tokio::test]
async fn test_redis_kv_nx_xx_conditions() -> Result<(), KvError> {
    let store = get_redis_store().await;

    let key = Key::from("test:cond:key");
    let val1 = Value::from("val1");
    let val2 = Value::from("val2");

    store.delete(&key).await?;

    // set XX when missing -> ConditionFailed
    assert_eq!(
        store
            .set(key.clone(), val1.clone(), SetOptions::new().if_exists())
            .await,
        Err(KvError::ConditionFailed)
    );

    // set NX when missing -> Ok
    store
        .set(key.clone(), val1.clone(), SetOptions::new().if_not_exists())
        .await?;

    // set NX when existing -> ConditionFailed
    assert_eq!(
        store
            .set(key.clone(), val2.clone(), SetOptions::new().if_not_exists())
            .await,
        Err(KvError::ConditionFailed)
    );

    // set XX when existing -> Ok
    store
        .set(key.clone(), val2.clone(), SetOptions::new().if_exists())
        .await?;
    assert_eq!(store.get(&key).await?, Some(val2));

    store.delete(&key).await?;

    Ok(())
}

#[tokio::test]
async fn test_redis_kv_ttl_expiration() -> Result<(), KvError> {
    let store = get_redis_store().await;

    let key = Key::from("test:ttl:key");
    let val = Value::from("ttl_val");

    store.delete(&key).await?;

    store
        .set(
            key.clone(),
            val,
            SetOptions::new().with_ttl(Duration::from_millis(200)),
        )
        .await?;

    assert!(store.exists(&key).await?);
    assert!(store.ttl(&key).await?.is_some());

    tokio::time::sleep(Duration::from_millis(300)).await;

    assert!(!store.exists(&key).await?);
    assert_eq!(store.get(&key).await?, None);

    Ok(())
}

#[tokio::test]
async fn test_redis_kv_batch_and_scan() -> Result<(), KvError> {
    let store = get_redis_store().await;

    let k1 = Key::from("test:scan:user:1");
    let k2 = Key::from("test:scan:user:2");
    let k3 = Key::from("test:scan:other:1");

    store.delete(&k1).await?;
    store.delete(&k2).await?;
    store.delete(&k3).await?;

    let ops = vec![
        BatchOp::Put {
            key: k1.clone(),
            value: Value::from("u1"),
            options: SetOptions::new(),
        },
        BatchOp::Put {
            key: k2.clone(),
            value: Value::from("u2"),
            options: SetOptions::new(),
        },
        BatchOp::Put {
            key: k3.clone(),
            value: Value::from("o1"),
            options: SetOptions::new(),
        },
    ];

    store.batch(ops).await?;

    let mut stream = store
        .scan(ScanOptions::new().with_prefix("test:scan:user:"))
        .await;
    let mut scanned = Vec::new();
    while let Some(res) = stream.next().await {
        scanned.push(res?.key.to_string());
    }

    scanned.sort();
    assert_eq!(scanned, vec!["test:scan:user:1", "test:scan:user:2"]);

    store.delete(&k1).await?;
    store.delete(&k2).await?;
    store.delete(&k3).await?;

    Ok(())
}

#[tokio::test]
async fn test_redis_kv_json_and_binary() -> Result<(), KvError> {
    let store = get_redis_store().await;

    let profile = UserProfile {
        id: 777,
        username: "antigravity".to_string(),
    };
    let key = Key::from("test:profile:key");
    let val = Value::from_json(&profile).unwrap();

    store.delete(&key).await?;
    store.set(key.clone(), val, SetOptions::new()).await?;

    let restored: UserProfile = store.get(&key).await?.unwrap().to_json().unwrap();
    assert_eq!(restored, profile);

    // Binary test
    let bin_key = Key::new(vec![0x00, 0xde, 0xad, 0xbe, 0xef]);
    let bin_val = Value::new(vec![200u8; 100_000]); // 100KB

    store.delete(&bin_key).await?;
    store
        .set(bin_key.clone(), bin_val.clone(), SetOptions::new())
        .await?;

    assert_eq!(store.get(&bin_key).await?, Some(bin_val));
    store.delete(&bin_key).await?;
    store.delete(&key).await?;

    Ok(())
}

#[tokio::test]
async fn test_redis_kv_clear() -> Result<(), KvError> {
    let store = get_redis_store().await;

    let k1 = Key::from("test:clear:k1");
    let k2 = Key::from("test:clear:k2");

    store
        .set(k1.clone(), Value::from("v1"), SetOptions::new())
        .await?;
    store
        .set(k2.clone(), Value::from("v2"), SetOptions::new())
        .await?;

    assert!(store.exists(&k1).await?);
    store.delete(&k1).await?;
    store.delete(&k2).await?;
    assert!(!store.exists(&k1).await?);
    assert!(!store.exists(&k2).await?);

    Ok(())
}

#[tokio::test]
async fn test_redis_kv_concurrency() -> Result<(), KvError> {
    let store = Arc::new(get_redis_store().await);
    let mut handles = Vec::new();

    for i in 0..10 {
        let store_clone = Arc::clone(&store);
        let handle = tokio::spawn(async move {
            let key = Key::from(format!("test:concurrent:{i}"));
            let val = Value::from(format!("val:{i}"));
            store_clone
                .set(key.clone(), val.clone(), SetOptions::new())
                .await
                .unwrap();
            let retrieved = store_clone.get(&key).await.unwrap();
            assert_eq!(retrieved, Some(val));
            store_clone.delete(&key).await.unwrap();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    Ok(())
}
