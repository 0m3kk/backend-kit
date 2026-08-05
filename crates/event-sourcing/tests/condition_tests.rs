#![allow(clippy::expect_used, clippy::unwrap_used)]

use event_sourcing::*;

#[test]
fn test_append_condition_evaluation() {
    let query = Query::item(QueryItem::new().with_tag("username:alice"));
    let condition = AppendCondition::new(query).after(SequencePosition::new(10));

    let matching_event = Event::new(
        "ev-1",
        "UsernameClaimed",
        serde_json::json!({"username": "alice"}),
        vec![Tag::key_value("username", "alice")],
    );

    let non_matching_event = Event::new(
        "ev-2",
        "UsernameClaimed",
        serde_json::json!({"username": "bob"}),
        vec![Tag::key_value("username", "bob")],
    );

    let seq_at_5 = SequencedEvent::new(SequencePosition::new(5), 1000, matching_event.clone());
    assert!(!condition.is_violated_by(&seq_at_5));

    let seq_at_10 = SequencedEvent::new(SequencePosition::new(10), 1000, matching_event.clone());
    assert!(!condition.is_violated_by(&seq_at_10));

    let seq_at_11 = SequencedEvent::new(SequencePosition::new(11), 1000, matching_event);
    assert!(condition.is_violated_by(&seq_at_11));

    let seq_at_11_other = SequencedEvent::new(SequencePosition::new(11), 1000, non_matching_event);
    assert!(!condition.is_violated_by(&seq_at_11_other));
}
