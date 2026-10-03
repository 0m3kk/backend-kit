//! Integration test for the EventStore trait and MockEventStore.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use event_store::{
    AppendCondition, Event, EventStore, MockEventStore, Query, QueryItem, SequencedEvent,
};
use futures_util::StreamExt;

#[tokio::test]
async fn mock_append() {
    let mut mock = MockEventStore::new();
    mock.expect_append().returning(|events, _| {
        Ok(events
            .into_iter()
            .enumerate()
            .map(|(i, event)| SequencedEvent {
                event,
                position: i as u64,
            })
            .collect())
    });

    let events = mock
        .append(
            vec![Event::new("created", serde_json::json!({"id": 1}), vec![])],
            None,
        )
        .await
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].position, 0);
}

#[tokio::test]
async fn mock_read() {
    let mut mock = MockEventStore::new();
    mock.expect_read()
        .returning(|_, _| Ok(Box::pin(futures_util::stream::empty())));

    let result = mock
        .read(
            Query::from_items([QueryItem::new(
                vec!["created".to_string()],
                vec!["product:1".into()],
            )]),
            Some(42),
        )
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await;

    assert!(result.is_empty());
}

#[tokio::test]
async fn mock_append_with_condition() {
    let mut mock = MockEventStore::new();
    mock.expect_append().returning(|_, _| Ok(vec![]));

    let condition = AppendCondition::new(
        Query::from_items([QueryItem::new(
            vec!["created".to_string()],
            vec!["product:1".into()],
        )]),
        Some(41),
    );

    let result = mock.append(vec![], Some(condition)).await.unwrap();
    assert!(result.is_empty());
}
