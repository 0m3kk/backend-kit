//! Integration test for the query module.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use event_store::{AppendCondition, Event, Query, QueryItem, Tags};

fn event(event_type: &str, tags: Tags) -> Event {
    Event::new(event_type, serde_json::Value::Null, tags)
}

fn item(types: &[&str], tags: Tags) -> QueryItem {
    QueryItem::new(types.iter().map(|t| t.to_string()).collect(), tags)
}

#[test]
fn all_query_matches_every_event() {
    let query = Query::all();

    assert!(query.matches(&event("created", vec![])));
    assert!(query.matches(&event("updated", vec!["product:1".into()])));
}

#[test]
fn query_items_are_combined_with_or() {
    let query = Query::from_items([item(&["created"], vec![]), item(&["deleted"], vec![])]);

    assert!(query.matches(&event("created", vec![])));
    assert!(query.matches(&event("deleted", vec![])));
    assert!(!query.matches(&event("updated", vec![])));
}

#[test]
fn query_item_matches_one_of_the_types() {
    let item = item(&["created", "updated"], vec![]);

    assert!(item.matches(&event("created", vec![])));
    assert!(item.matches(&event("updated", vec![])));
    assert!(!item.matches(&event("deleted", vec![])));
}

#[test]
fn query_item_matches_all_of_the_tags() {
    let item = item(&[], vec!["tag1".into(), "tag2".into()]);

    assert!(item.matches(&event("created", vec!["tag1".into(), "tag2".into()])));
    assert!(item.matches(&event(
        "created",
        vec!["tag1".into(), "tag2".into(), "tag3".into()]
    )));
    assert!(!item.matches(&event("created", vec!["tag1".into()])));
    assert!(!item.matches(&event("created", vec![])));
}

#[test]
fn spec_example_query_items_match_as_documented() {
    let query = Query::from_items([
        item(&["EventType1", "EventType2"], vec![]),
        item(&[], vec!["tag1".into(), "tag2".into()]),
        item(
            &["EventType2", "EventType3"],
            vec!["tag1".into(), "tag3".into()],
        ),
    ]);

    assert!(query.matches(&event("EventType1", vec![])));
    assert!(query.matches(&event("EventType2", vec!["unrelated".into()])));
    assert!(query.matches(&event("AnyType", vec!["tag1".into(), "tag2".into()])));
    assert!(query.matches(&event("EventType3", vec!["tag1".into(), "tag3".into()])));
    assert!(!query.matches(&event("EventType4", vec!["tag1".into()])));
    assert!(!query.matches(&event("EventType4", vec!["tag3".into()])));
}

#[test]
fn append_condition_carries_query_and_after() {
    let condition = AppendCondition::new(Query::from_items([item(&["created"], vec![])]), Some(41));

    assert_eq!(condition.after, Some(41));
    assert!(
        condition
            .fail_if_events_match
            .matches(&event("created", vec![]))
    );
    assert!(
        !condition
            .fail_if_events_match
            .matches(&event("updated", vec![]))
    );
}
