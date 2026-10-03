//! Integration test for the event module.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use event_store::{Event, SequencePosition, SequencedEvent, Tags};

#[test]
fn event_carries_type_data_and_tags() {
    let event = Event::new(
        "created",
        serde_json::json!({"id": 1, "name": "widget"}),
        vec!["product:1".into()],
    );

    assert_eq!(event.event_type, "created");
    assert_eq!(event.data, serde_json::json!({"id": 1, "name": "widget"}));
    assert_eq!(event.tags, vec!["product:1".to_string()]);
}

#[test]
fn sequenced_event_embeds_event_and_position() {
    let event = Event::new("created", serde_json::Value::Null, vec![]);
    let sequenced = SequencedEvent {
        event: event.clone(),
        position: 42,
    };

    assert_eq!(sequenced.event, event);
    assert_eq!(sequenced.position, 42);
}

#[test]
fn tags_and_sequence_position_are_public_types() {
    let tags: Tags = vec!["a".into(), "b".into()];
    let position: SequencePosition = 7;

    assert_eq!(tags.len(), 2);
    assert_eq!(position, 7);
}
