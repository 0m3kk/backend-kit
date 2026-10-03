//! Integration test for the error module.

use event_store::{AppendConditionError, Error, Event, SequencedEvent};

fn sequenced_event(event_type: &str) -> SequencedEvent {
    SequencedEvent {
        event: Event::new(event_type, serde_json::Value::Null, vec![]),
        position: 0,
    }
}

#[test]
fn store_error_is_constructible() {
    let error: Error<&str> = Error::Store("boom");

    assert!(matches!(error, Error::Store("boom")));
}

#[test]
fn condition_error_is_constructible() {
    let error: Error<&str> = Error::Condition(AppendConditionError {
        conflicting_events: vec![sequenced_event("created")],
    });

    match error {
        Error::Condition(condition) => {
            assert_eq!(condition.conflicting_events.len(), 1);
            assert_eq!(condition.conflicting_events[0].event.event_type, "created");
        }
        Error::Store(_) => panic!("expected condition error"),
    }
}
