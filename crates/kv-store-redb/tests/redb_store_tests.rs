use futures_util::StreamExt;
use kv_store::{BatchOp, Key, KvError, KvStore, ScanOptions, SetOptions, Value};
use kv_store_redb::RedbKvStore;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tempfile::NamedTempFile;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Product {
    id: String,
    price: u64,
}

#[tokio::test]
async fn test_redb_basic_get_set_delete() -> Result<(), KvError> {
    let tmp = NamedTempFile::new().map_err(|e| KvError::StoreError(e.to_string()))?;
    let store = RedbKvStore::open(tmp.path())?;

    let key = Key::from("k1");
    let val = Value::from("v1");

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
async fn test_redb_nx_xx_conditions() -> Result<(), KvError> {
    let tmp = NamedTempFile::new().map_err(|e| KvError::StoreError(e.to_string()))?;
    let store = RedbKvStore::open(tmp.path())?;

    let key = Key::from("key_cond");
    let val1 = Value::from("val1");
    let val2 = Value::from("val2");

    // set XX when key doesn't exist -> fail
    assert_eq!(
        store
            .set(key.clone(), val1.clone(), SetOptions::new().if_exists())
            .await,
        Err(KvError::ConditionFailed)
    );

    // set NX when key doesn't exist -> success
    store
        .set(key.clone(), val1.clone(), SetOptions::new().if_not_exists())
        .await?;

    // set NX when key exists -> fail
    assert_eq!(
        store
            .set(key.clone(), val2.clone(), SetOptions::new().if_not_exists())
            .await,
        Err(KvError::ConditionFailed)
    );

    // set XX when key exists -> success
    store
        .set(key.clone(), val2.clone(), SetOptions::new().if_exists())
        .await?;
    assert_eq!(store.get(&key).await?, Some(val2));

    Ok(())
}

#[tokio::test]
async fn test_redb_ttl_expiration() -> Result<(), KvError> {
    let tmp = NamedTempFile::new().map_err(|e| KvError::StoreError(e.to_string()))?;
    let store = RedbKvStore::open(tmp.path())?;

    let key = Key::from("temp_key");
    let val = Value::from("temp_val");

    store
        .set(
            key.clone(),
            val,
            SetOptions::new().with_ttl(Duration::from_millis(50)),
        )
        .await?;

    assert!(store.exists(&key).await?);
    assert!(store.ttl(&key).await?.is_some());

    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(!store.exists(&key).await?);
    assert_eq!(store.get(&key).await?, None);
    assert_eq!(store.ttl(&key).await?, None);

    Ok(())
}

#[tokio::test]
async fn test_redb_batch_and_atomic_failure() -> Result<(), KvError> {
    let tmp = NamedTempFile::new().map_err(|e| KvError::StoreError(e.to_string()))?;
    let store = RedbKvStore::open(tmp.path())?;

    let ops = vec![
        BatchOp::Put {
            key: Key::from("item:1"),
            value: Value::from("val1"),
            options: SetOptions::new(),
        },
        BatchOp::Put {
            key: Key::from("item:2"),
            value: Value::from("val2"),
            options: SetOptions::new(),
        },
    ];

    store.batch(ops).await?;

    assert_eq!(
        store.get(&Key::from("item:1")).await?,
        Some(Value::from("val1"))
    );
    assert_eq!(
        store.get(&Key::from("item:2")).await?,
        Some(Value::from("val2"))
    );

    // Batch with failing condition -> should abort
    let failing_ops = vec![
        BatchOp::Put {
            key: Key::from("item:3"),
            value: Value::from("val3"),
            options: SetOptions::new(),
        },
        BatchOp::Put {
            key: Key::from("item:1"),
            value: Value::from("new_val1"),
            options: SetOptions::new().if_not_exists(), // item:1 exists -> fail
        },
    ];

    assert_eq!(
        store.batch(failing_ops).await,
        Err(KvError::ConditionFailed)
    );
    assert_eq!(store.get(&Key::from("item:3")).await?, None);
    assert_eq!(
        store.get(&Key::from("item:1")).await?,
        Some(Value::from("val1"))
    );

    Ok(())
}

#[tokio::test]
async fn test_redb_scan_options_range_and_reverse() -> Result<(), KvError> {
    let tmp = NamedTempFile::new().map_err(|e| KvError::StoreError(e.to_string()))?;
    let store = RedbKvStore::open(tmp.path())?;

    store
        .set(Key::from("a"), Value::from("1"), SetOptions::new())
        .await?;
    store
        .set(Key::from("b"), Value::from("2"), SetOptions::new())
        .await?;
    store
        .set(Key::from("c"), Value::from("3"), SetOptions::new())
        .await?;
    store
        .set(Key::from("d"), Value::from("4"), SetOptions::new())
        .await?;

    // Range scan [b, d] with limit 2
    let scan_opts = ScanOptions::new()
        .with_range(Some(Key::from("b")), Some(Key::from("d")))
        .with_limit(2);

    let mut stream = store.scan(scan_opts).await;
    let mut keys = Vec::new();
    while let Some(res) = stream.next().await {
        keys.push(res?.key.to_string());
    }
    assert_eq!(keys, vec!["b", "c"]);

    // Reverse scan
    let mut rev_stream = store.scan(ScanOptions::new().reverse()).await;
    let mut rev_keys = Vec::new();
    while let Some(res) = rev_stream.next().await {
        rev_keys.push(res?.key.to_string());
    }
    assert_eq!(rev_keys, vec!["d", "c", "b", "a"]);

    Ok(())
}

#[tokio::test]
async fn test_redb_json_serialization_and_binary() -> Result<(), KvError> {
    let tmp = NamedTempFile::new().map_err(|e| KvError::StoreError(e.to_string()))?;
    let store = RedbKvStore::open(tmp.path())?;

    let product = Product {
        id: "prod-100".to_string(),
        price: 499,
    };

    let key = Key::from("product:100");
    let val = Value::from_json(&product).expect("serialization failed");

    store.set(key.clone(), val, SetOptions::new()).await?;

    let restored: Product = store
        .get(&key)
        .await?
        .expect("product key should exist")
        .to_json()
        .expect("deserialization failed");

    assert_eq!(restored, product);

    // Large binary test
    let bin_key = Key::new(vec![0x00, 0xab, 0xcd]);
    let bin_val = Value::new(vec![255u8; 100_000]);
    store
        .set(bin_key.clone(), bin_val.clone(), SetOptions::new())
        .await?;

    assert_eq!(store.get(&bin_key).await?, Some(bin_val));

    Ok(())
}

#[tokio::test]
async fn test_redb_clear() -> Result<(), KvError> {
    let tmp = NamedTempFile::new().map_err(|e| KvError::StoreError(e.to_string()))?;
    let store = RedbKvStore::open(tmp.path())?;

    store
        .set(Key::from("k1"), Value::from("v1"), SetOptions::new())
        .await?;
    store
        .set(Key::from("k2"), Value::from("v2"), SetOptions::new())
        .await?;

    assert!(store.exists(&Key::from("k1")).await?);
    store.clear().await?;
    assert!(!store.exists(&Key::from("k1")).await?);
    assert!(!store.exists(&Key::from("k2")).await?);

    Ok(())
}

#[tokio::test]
async fn test_redb_concurrency() -> Result<(), KvError> {
    let tmp = NamedTempFile::new().map_err(|e| KvError::StoreError(e.to_string()))?;
    let store = Arc::new(RedbKvStore::open(tmp.path())?);
    let mut handles = Vec::new();

    for i in 0..10 {
        let store_clone = Arc::clone(&store);
        let handle = tokio::spawn(async move {
            let key = Key::from(format!("redb:user:{i}"));
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

    Ok(())
}

#[tokio::test]
async fn test_redb_clean_expired() -> Result<(), KvError> {
    let tmp = NamedTempFile::new().map_err(|e| KvError::StoreError(e.to_string()))?;
    let store = RedbKvStore::open(tmp.path())?;

    store
        .set(
            Key::from("k_exp"),
            Value::from("v1"),
            SetOptions::new().with_ttl(Duration::from_millis(50)),
        )
        .await?;
    store
        .set(Key::from("k_valid"), Value::from("v2"), SetOptions::new())
        .await?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let removed = store.clean_expired(Some(10)).await?;
    assert_eq!(removed, 1);
    assert!(!store.exists(&Key::from("k_exp")).await?);
    assert!(store.exists(&Key::from("k_valid")).await?);

    Ok(())
}
