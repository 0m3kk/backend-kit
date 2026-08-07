use attempt_policy::{AttemptPolicy, AttemptTracker, KvAttemptTracker};
use kv_store::memory::MemoryKvStore;
use std::time::Duration;

#[tokio::test]
async fn test_kv_attempt_tracker() {
    let store = MemoryKvStore::new();
    let tracker = KvAttemptTracker::new(store);
    let policy = AttemptPolicy {
        max_attempts: 3,
        lockout_duration: Duration::from_secs(60),
        window_duration: Some(Duration::from_secs(60)),
        reset_on_success: true,
    };
    let key = "kv_tracker_user@example.com";

    let status1 = tracker.check_status(key, &policy).await.unwrap();
    assert!(status1.is_allowed());

    tracker.record_failed_attempt(key, &policy).await.unwrap();
    tracker.record_failed_attempt(key, &policy).await.unwrap();

    let status2 = tracker.check_status(key, &policy).await.unwrap();
    assert!(status2.is_allowed());

    let status3 = tracker.record_failed_attempt(key, &policy).await.unwrap();
    assert!(status3.is_locked());

    tracker.reset(key).await.unwrap();
    let status_after_reset = tracker.check_status(key, &policy).await.unwrap();
    assert!(status_after_reset.is_allowed());
}
