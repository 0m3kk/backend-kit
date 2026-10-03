//! Integration test for the value module.

use kv_store::{ReplacedValue, StoredValue};
use serde_json::json;
use time::OffsetDateTime;

#[test]
fn stored_value_carries_key_value_and_timestamps() {
    let now = OffsetDateTime::now_utc();
    let stored = StoredValue {
        key: "key1".to_string(),
        value: json!({"a": 1}),
        created_at: Some(now),
        updated_at: Some(now),
    };

    assert_eq!(stored.key, "key1");
    assert_eq!(stored.value, json!({"a": 1}));
    assert_eq!(stored.created_at, Some(now));
    assert_eq!(stored.updated_at, Some(now));
}

#[test]
fn replaced_value_carries_previous_and_current() {
    let now = OffsetDateTime::now_utc();
    let replaced = ReplacedValue {
        key: "key1".to_string(),
        value: json!({"a": 2}),
        previous: Some(json!({"a": 1})),
        created_at: Some(now),
        updated_at: Some(now),
    };

    assert_eq!(replaced.key, "key1");
    assert_eq!(replaced.value, json!({"a": 2}));
    assert_eq!(replaced.previous, Some(json!({"a": 1})));
}
