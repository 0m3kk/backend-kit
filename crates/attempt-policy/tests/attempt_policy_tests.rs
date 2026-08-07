use attempt_policy::{AttemptPolicy, AttemptStatus};
use std::time::Duration;

#[test]
fn test_attempt_policy_presets_and_builder() {
    let default_policy = AttemptPolicy::default();
    assert_eq!(default_policy.max_attempts, 5);
    assert_eq!(default_policy.lockout_duration, Duration::from_secs(900));

    let strict_policy = AttemptPolicy::strict();
    assert_eq!(strict_policy.max_attempts, 3);
    assert_eq!(strict_policy.lockout_duration, Duration::from_secs(1800));

    let relaxed_policy = AttemptPolicy::relaxed();
    assert_eq!(relaxed_policy.max_attempts, 10);
    assert_eq!(relaxed_policy.lockout_duration, Duration::from_secs(300));

    let custom_policy = AttemptPolicy::builder()
        .max_attempts(7)
        .lockout_duration(Duration::from_secs(600))
        .window_duration(Some(Duration::from_secs(600)))
        .reset_on_success(false)
        .build();

    assert_eq!(custom_policy.max_attempts, 7);
    assert_eq!(custom_policy.lockout_duration, Duration::from_secs(600));
    assert!(!custom_policy.reset_on_success);
}

#[test]
fn test_attempt_status_methods() {
    let allowed = AttemptStatus::Allowed {
        attempts_made: 2,
        attempts_remaining: 3,
    };
    assert!(allowed.is_allowed());
    assert!(!allowed.is_locked());

    let locked = AttemptStatus::LockedOut {
        locked_until: std::time::SystemTime::now() + Duration::from_secs(60),
        retry_after: Duration::from_secs(60),
    };
    assert!(!locked.is_allowed());
    assert!(locked.is_locked());
}
