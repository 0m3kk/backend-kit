use cqrs::{CheckpointStore, CheckpointStoreTx, KvCheckpointStore, KvCheckpointStoreTx};
use event_sourcing::SequencePosition;
use kv_store::memory::MemoryKvStore;

#[tokio::test]
async fn test_kv_checkpoint_store_basic_persistence() {
    let kv_store = MemoryKvStore::new();
    let checkpoint_store = KvCheckpointStore::new(kv_store);

    assert_eq!(
        checkpoint_store
            .get_position("user_profiles")
            .await
            .unwrap(),
        None
    );

    checkpoint_store
        .save_position("user_profiles", SequencePosition::new(42))
        .await
        .unwrap();

    assert_eq!(
        checkpoint_store
            .get_position("user_profiles")
            .await
            .unwrap(),
        Some(SequencePosition::new(42))
    );
}

#[tokio::test]
async fn test_kv_checkpoint_store_custom_prefix() {
    let kv_store = MemoryKvStore::new();
    let checkpoint_store = KvCheckpointStore::with_prefix(kv_store, "custom:prefix:");

    assert_eq!(
        checkpoint_store
            .get_position("user_profiles")
            .await
            .unwrap(),
        None
    );

    checkpoint_store
        .save_position("user_profiles", SequencePosition::new(100))
        .await
        .unwrap();

    assert_eq!(
        checkpoint_store
            .get_position("user_profiles")
            .await
            .unwrap(),
        Some(SequencePosition::new(100))
    );
}

#[tokio::test]
async fn test_kv_checkpoint_store_tx_persistence() {
    let kv_store = MemoryKvStore::new();
    let checkpoint_store = KvCheckpointStoreTx::new(kv_store);
    let mut conn = ();

    assert_eq!(
        checkpoint_store
            .get_position_tx(&mut conn, "user_profiles")
            .await
            .unwrap(),
        None
    );

    checkpoint_store
        .save_position_tx(&mut conn, "user_profiles", SequencePosition::new(99))
        .await
        .unwrap();

    assert_eq!(
        checkpoint_store
            .get_position_tx(&mut conn, "user_profiles")
            .await
            .unwrap(),
        Some(SequencePosition::new(99))
    );
}

#[tokio::test]
async fn test_checkpoint_store_arc_blanket_implementation() {
    use std::sync::Arc;

    let kv_store = MemoryKvStore::new();
    let checkpoint_store: Arc<dyn CheckpointStore> = Arc::new(KvCheckpointStore::new(kv_store));

    assert_eq!(
        checkpoint_store.get_position("view_arc").await.unwrap(),
        None
    );

    checkpoint_store
        .save_position("view_arc", SequencePosition::new(500))
        .await
        .unwrap();

    assert_eq!(
        checkpoint_store.get_position("view_arc").await.unwrap(),
        Some(SequencePosition::new(500))
    );
}

#[tokio::test]
async fn test_checkpoint_store_tx_arc_blanket_implementation() {
    use std::sync::Arc;

    let kv_store = MemoryKvStore::new();
    let checkpoint_store: Arc<dyn CheckpointStoreTx<()>> =
        Arc::new(KvCheckpointStoreTx::new(kv_store));
    let mut conn = ();

    assert_eq!(
        checkpoint_store
            .get_position_tx(&mut conn, "view_arc_tx")
            .await
            .unwrap(),
        None
    );

    checkpoint_store
        .save_position_tx(&mut conn, "view_arc_tx", SequencePosition::new(750))
        .await
        .unwrap();

    assert_eq!(
        checkpoint_store
            .get_position_tx(&mut conn, "view_arc_tx")
            .await
            .unwrap(),
        Some(SequencePosition::new(750))
    );
}

#[tokio::test]
async fn test_checkpoint_store_multiple_isolated_views() {
    let kv_store = MemoryKvStore::new();
    let checkpoint_store = KvCheckpointStore::new(kv_store);

    checkpoint_store
        .save_position("view_a", SequencePosition::new(10))
        .await
        .unwrap();
    checkpoint_store
        .save_position("view_b", SequencePosition::new(20))
        .await
        .unwrap();
    checkpoint_store
        .save_position("view_c", SequencePosition::new(30))
        .await
        .unwrap();

    assert_eq!(
        checkpoint_store.get_position("view_a").await.unwrap(),
        Some(SequencePosition::new(10))
    );
    assert_eq!(
        checkpoint_store.get_position("view_b").await.unwrap(),
        Some(SequencePosition::new(20))
    );
    assert_eq!(
        checkpoint_store.get_position("view_c").await.unwrap(),
        Some(SequencePosition::new(30))
    );
    assert_eq!(
        checkpoint_store.get_position("view_d").await.unwrap(),
        None
    );
}

#[tokio::test]
async fn test_checkpoint_store_invalid_parse_handling() {
    use kv_store::{Key, KvStore, SetOptions, Value};

    let kv_store = MemoryKvStore::new();
    let checkpoint_store = KvCheckpointStore::new(kv_store.clone());

    // Inject non-integer string into the KV key
    let key = Key::new("cqrs:checkpoint:corrupted_view");
    kv_store
        .set(key, Value::from("not_a_number"), SetOptions::default())
        .await
        .unwrap();

    let err = checkpoint_store
        .get_position("corrupted_view")
        .await
        .unwrap_err();
    match err {
        cqrs::CheckpointError::Parse(msg) => {
            assert!(msg.contains("invalid digit") || msg.contains("ParseIntError"));
        }
        _ => panic!("Expected CheckpointError::Parse"),
    }
}


