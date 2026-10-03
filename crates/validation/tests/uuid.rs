//! Integration tests for UUID validation.

use validation::validate_uuid_v7;

#[test]
fn validates_generated_uuid_v7() {
    let id = uuid::Uuid::now_v7().to_string();
    assert!(validate_uuid_v7(&id).is_ok());
}

#[test]
fn rejects_uuid_v4() {
    let id = "00000000-0000-4000-8000-000000000000";
    assert!(validate_uuid_v7(id).is_err());
}
