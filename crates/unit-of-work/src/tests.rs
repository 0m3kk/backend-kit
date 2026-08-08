use std::time::Duration;
use crate::runner::{IsolationLevel, RetryPolicy, is_retryable_sql_error};

#[test]
fn test_isolation_level_sql_begin() {
    assert_eq!(IsolationLevel::ReadCommitted.sql_begin(), "BEGIN ISOLATION LEVEL READ COMMITTED");
    assert_eq!(IsolationLevel::RepeatableRead.sql_begin(), "BEGIN ISOLATION LEVEL REPEATABLE READ");
    assert_eq!(IsolationLevel::Serializable.sql_begin(), "BEGIN ISOLATION LEVEL SERIALIZABLE");
}

#[test]
fn test_retry_policy_exponential_backoff() {
    let policy = RetryPolicy {
        max_attempts: 5,
        initial_delay: Duration::from_millis(10),
        max_delay: Duration::from_millis(100),
    };

    assert_eq!(policy.backoff_for(1), Duration::from_millis(10));
    assert_eq!(policy.backoff_for(2), Duration::from_millis(20));
    assert_eq!(policy.backoff_for(3), Duration::from_millis(40));
    assert_eq!(policy.backoff_for(4), Duration::from_millis(80));
    assert_eq!(policy.backoff_for(5), Duration::from_millis(100)); // clamped by max_delay
}

#[test]
fn test_is_retryable_sql_error() {
    assert!(is_retryable_sql_error("ERROR: 40001: could not serialize access due to read/write dependencies"));
    assert!(is_retryable_sql_error("ERROR: 40P01: deadlock detected"));
    assert!(is_retryable_sql_error("serialization failure occurred"));
    assert!(!is_retryable_sql_error("ERROR: 23505: unique violation"));
    assert!(!is_retryable_sql_error("syntax error"));
}
