//! Integration test for the error module.

use kv_store::{Error, PutConditionError};

#[test]
fn store_error_is_constructible() {
    let error: Error<&str> = Error::Store("boom");
    assert!(matches!(error, Error::Store("boom")));
}

#[test]
fn condition_error_is_constructible() {
    let error: Error<&str> = Error::Condition(PutConditionError {
        current: Some(serde_json::json!({"n": 1})),
    });

    match error {
        Error::Condition(condition) => {
            assert_eq!(condition.current, Some(serde_json::json!({"n": 1})));
        }
        Error::Store(_) => panic!("expected condition error"),
    }
}
