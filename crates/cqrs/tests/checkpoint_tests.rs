use cqrs::{CheckpointStore, KvCheckpointStore};
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
