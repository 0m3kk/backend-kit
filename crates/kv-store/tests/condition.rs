//! Integration test for the condition module.

use kv_store::PutCondition;

#[test]
fn condition_absent_and_value_equality() {
    let absent = PutCondition::Absent;
    assert_eq!(absent, PutCondition::Absent);

    let val = PutCondition::Value(serde_json::json!({"test": 123}));
    assert_eq!(val, PutCondition::Value(serde_json::json!({"test": 123})));
    assert_ne!(absent, val);
}
