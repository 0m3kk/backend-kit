use attempt_policy::{AttemptError, AttemptManager, AttemptPolicy, KvAttemptTracker};
use kv_store::memory::MemoryKvStore;
use std::time::Duration;

#[tokio::test]
async fn test_attempt_manager_flow() {
    let policy = AttemptPolicy {
        max_attempts: 3,
        lockout_duration: Duration::from_secs(60),
        window_duration: Some(Duration::from_secs(60)),
        reset_on_success: true,
    };
    let tracker = KvAttemptTracker::new(MemoryKvStore::new());
    let manager = AttemptManager::new(policy, tracker);
    let key = "manager_user@example.com";

    // 1. Initial check
    let status = manager.check_attempt(key).await.unwrap();
    assert!(status.is_allowed());

    // 2. Failed attempt 1 & 2
    manager.record_failed_attempt(key).await.unwrap();
    manager.record_failed_attempt(key).await.unwrap();

    // 3. Failed attempt 3 -> triggers MaxAttemptsExceeded
    let err = manager.record_failed_attempt(key).await.unwrap_err();
    assert!(matches!(err, AttemptError::MaxAttemptsExceeded { .. }));

    // 4. Lockout check
    let err_check = manager.check_attempt(key).await.unwrap_err();
    assert!(matches!(
        err_check,
        AttemptError::MaxAttemptsExceeded { .. }
    ));

    // 5. Unlock
    manager.unlock(key).await.unwrap();
    let status_unlocked = manager.check_attempt(key).await.unwrap();
    assert!(status_unlocked.is_allowed());
}

#[tokio::test]
async fn test_attempt_manager_record_success() {
    let policy = AttemptPolicy::default();
    let tracker = KvAttemptTracker::new(MemoryKvStore::new());
    let manager = AttemptManager::new(policy, tracker);
    let key = "manager_success_user@example.com";

    manager.record_failed_attempt(key).await.unwrap();
    manager.record_failed_attempt(key).await.unwrap();

    manager.record_success(key).await.unwrap();

    let status = manager.check_attempt(key).await.unwrap();
    assert!(status.is_allowed());
}
