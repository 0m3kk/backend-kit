#![allow(clippy::expect_used, clippy::unwrap_used)]

use event_sourcing::*;

#[test]
fn test_event_type_and_id_creation() {
    let id = EventId::new("evt-100");
    assert_eq!(id.as_str(), "evt-100");
    assert_eq!(id.to_string(), "evt-100");

    let event_type = EventType::new("OrderCreated");
    assert_eq!(event_type.as_str(), "OrderCreated");
    assert_eq!(event_type.to_string(), "OrderCreated");

    let tag = Tag::key_value("order", "ord-1");
    assert_eq!(tag.as_str(), "order:ord-1");
}

#[test]
fn test_sequence_position_behavior() {
    let pos0 = SequencePosition::ZERO;
    assert_eq!(pos0.value(), 0);
    assert_eq!(pos0.to_string(), "0");

    let pos1 = pos0.next();
    assert_eq!(pos1.value(), 1);
    assert!(pos1 > pos0);
}
