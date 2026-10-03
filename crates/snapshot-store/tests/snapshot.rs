use serde_json::json;
use snapshot_store::Snapshot;

#[test]
fn snapshot_stores_position_and_state() {
    let s = Snapshot::new(Some(42), json!({"count": 1}));
    assert_eq!(s.position, Some(42));
    assert_eq!(s.state, json!({"count": 1}));
}
