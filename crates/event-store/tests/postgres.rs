//! DCB compliance suite for the postgres event store, run against a real
//! database via `#[sqlx::test]`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use event_store::{
    AppendCondition, Error, Event, EventStore, PostgresEventStore, Query, QueryItem, SequencedEvent,
};
use std::time::Duration;

use futures_util::StreamExt;
use sqlx::PgPool;

fn event(event_type: &str, tags: Vec<String>) -> Event {
    Event::new(event_type, serde_json::Value::Null, tags)
}

fn item(types: &[&str], tags: Vec<String>) -> QueryItem {
    QueryItem::new(types.iter().map(|t| t.to_string()).collect(), tags)
}

async fn read_all(
    store: &PostgresEventStore,
    query: Query,
    position: Option<u64>,
) -> Vec<SequencedEvent> {
    store
        .read(query, position)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect()
}

fn assert_event_types(events: &[SequencedEvent], expected: &[&str]) {
    let actual: Vec<&str> = events.iter().map(|e| e.event.event_type.as_str()).collect();
    assert_eq!(actual, expected);
}

#[sqlx::test]
async fn append_waits_for_the_store_wide_lock(pool: PgPool) {
    let store = PostgresEventStore::new(pool.clone());
    let mut conn = pool.acquire().await.unwrap();

    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(event_store::APPEND_LOCK_ID)
        .execute(&mut *conn)
        .await
        .unwrap();

    let append = tokio::spawn(async move {
        store
            .append(vec![event("created", vec!["product:1".into()])], None)
            .await
    });

    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        !append.is_finished(),
        "append must wait for the store-wide append lock"
    );

    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(event_store::APPEND_LOCK_ID)
        .execute(&mut *conn)
        .await
        .unwrap();

    let result = tokio::time::timeout(Duration::from_secs(5), append)
        .await
        .expect("append did not finish after releasing the lock")
        .expect("append task panicked");

    assert!(result.is_ok(), "append failed: {result:?}");
}

#[sqlx::test]
async fn read_all_returns_events_in_position_order(pool: PgPool) {
    let store = PostgresEventStore::new(pool);
    store
        .append(
            vec![
                event("created", vec!["product:1".into()]),
                event("updated", vec!["product:1".into()]),
            ],
            None,
        )
        .await
        .unwrap();

    let result = read_all(&store, Query::all(), None).await;

    assert_event_types(&result, &["created", "updated"]);
    assert_eq!(
        result.iter().map(|e| e.position).collect::<Vec<_>>(),
        vec![0, 1]
    );
}

#[sqlx::test]
async fn read_filters_by_event_type(pool: PgPool) {
    let store = PostgresEventStore::new(pool);
    store
        .append(
            vec![
                event("created", vec![]),
                event("updated", vec![]),
                event("deleted", vec![]),
            ],
            None,
        )
        .await
        .unwrap();

    let result = read_all(
        &store,
        Query::from_items([item(&["created", "updated"], vec![])]),
        None,
    )
    .await;

    assert_event_types(&result, &["created", "updated"]);
}

#[sqlx::test]
async fn read_filters_by_tags(pool: PgPool) {
    let store = PostgresEventStore::new(pool);
    store
        .append(
            vec![
                event("created", vec!["product:1".into(), "tag1".into()]),
                event("updated", vec!["product:1".into()]),
                event("created", vec!["product:2".into()]),
            ],
            None,
        )
        .await
        .unwrap();

    let result = read_all(
        &store,
        Query::from_items([item(&[], vec!["product:1".into(), "tag1".into()])]),
        None,
    )
    .await;

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].event.event_type, "created");
}

#[sqlx::test]
async fn read_starts_from_sequence_position(pool: PgPool) {
    let store = PostgresEventStore::new(pool);
    store
        .append(
            vec![event("a", vec![]), event("b", vec![]), event("c", vec![])],
            None,
        )
        .await
        .unwrap();

    let result = read_all(&store, Query::all(), Some(1)).await;

    assert_event_types(&result, &["b", "c"]);
}

#[sqlx::test]
async fn concurrent_conditional_appends_serialize_and_retry(pool: PgPool) {
    let store = PostgresEventStore::new(pool);
    let mut handles = Vec::new();

    for i in 0..8 {
        let store = store.clone();
        handles.push(tokio::spawn(async move {
            let tag = format!("product:{i}");
            store
                .append(
                    vec![event("created", vec![tag.clone()])],
                    Some(AppendCondition::new(
                        Query::from_items([item(&["created"], vec![tag])]),
                        None,
                    )),
                )
                .await
        }));
    }

    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    let events = read_all(&store, Query::all(), None).await;
    assert_eq!(events.len(), 8);
}

#[sqlx::test]
async fn query_items_are_combined_with_or(pool: PgPool) {
    let store = PostgresEventStore::new(pool);
    store
        .append(
            vec![event("a", vec![]), event("b", vec![]), event("c", vec![])],
            None,
        )
        .await
        .unwrap();

    let result = read_all(
        &store,
        Query::from_items([item(&["a"], vec![]), item(&["c"], vec![])]),
        None,
    )
    .await;

    assert_event_types(&result, &["a", "c"]);
}

#[sqlx::test]
async fn append_condition_fails_when_event_matches(pool: PgPool) {
    let store = PostgresEventStore::new(pool);
    store
        .append(vec![event("created", vec!["product:1".into()])], None)
        .await
        .unwrap();

    let result = store
        .append(
            vec![event("updated", vec!["product:1".into()])],
            Some(AppendCondition::new(
                Query::from_items([item(&["created"], vec!["product:1".into()])]),
                None,
            )),
        )
        .await;

    match result {
        Err(Error::Condition(error)) => {
            assert_eq!(error.conflicting_events.len(), 1);
            assert_eq!(error.conflicting_events[0].position, 0);
            assert_eq!(error.conflicting_events[0].event.event_type, "created");
        }
        other => panic!("expected condition error, got {other:?}"),
    }
}

#[sqlx::test]
async fn append_condition_ignores_events_at_or_before_after(pool: PgPool) {
    let store = PostgresEventStore::new(pool);
    store
        .append(vec![event("created", vec!["product:1".into()])], None)
        .await
        .unwrap();

    let result = store
        .append(
            vec![event("updated", vec!["product:1".into()])],
            Some(AppendCondition::new(
                Query::from_items([item(&["created"], vec!["product:1".into()])]),
                Some(0),
            )),
        )
        .await;

    assert!(result.is_ok());
}

#[sqlx::test]
async fn append_condition_fails_on_event_after_position(pool: PgPool) {
    let store = PostgresEventStore::new(pool);
    store
        .append(
            vec![
                event("created", vec!["product:1".into()]),
                event("updated", vec!["product:1".into()]),
            ],
            None,
        )
        .await
        .unwrap();

    let result = store
        .append(
            vec![event("deleted", vec!["product:1".into()])],
            Some(AppendCondition::new(
                Query::from_items([item(&["updated"], vec!["product:1".into()])]),
                Some(0),
            )),
        )
        .await;

    match result {
        Err(Error::Condition(error)) => {
            assert_eq!(error.conflicting_events[0].position, 1);
        }
        other => panic!("expected condition error, got {other:?}"),
    }
}

#[sqlx::test]
async fn append_condition_succeeds_when_nothing_matches(pool: PgPool) {
    let store = PostgresEventStore::new(pool);
    store
        .append(vec![event("created", vec![])], None)
        .await
        .unwrap();

    let result = store
        .append(
            vec![event("other", vec![])],
            Some(AppendCondition::new(
                Query::from_items([item(&["never-happened"], vec![])]),
                None,
            )),
        )
        .await;

    assert!(result.is_ok());
}

#[sqlx::test]
async fn append_condition_with_all_query_fails_on_any_event(pool: PgPool) {
    let store = PostgresEventStore::new(pool);
    store
        .append(vec![event("created", vec![])], None)
        .await
        .unwrap();

    let result = store
        .append(
            vec![event("updated", vec![])],
            Some(AppendCondition::new(Query::all(), None)),
        )
        .await;

    assert!(matches!(result, Err(Error::Condition(_))));
}

#[sqlx::test]
async fn append_persists_json_data(pool: PgPool) {
    let store = PostgresEventStore::new(pool);
    store
        .append(
            vec![Event::new(
                "created",
                serde_json::json!({"id": 42, "name": "widget"}),
                vec!["product:1".into()],
            )],
            None,
        )
        .await
        .unwrap();

    let result = read_all(&store, Query::all(), None).await;

    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0].event.data,
        serde_json::json!({"id": 42, "name": "widget"})
    );
}

#[sqlx::test]
async fn append_persists_metadata(pool: PgPool) {
    let store = PostgresEventStore::new(pool);
    store
        .append(
            vec![
                Event::new("created", serde_json::Value::Null, vec![])
                    .with_metadata(serde_json::json!({"trace_id": "abc"})),
            ],
            None,
        )
        .await
        .unwrap();

    let result = read_all(&store, Query::all(), None).await;

    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0].event.metadata,
        serde_json::json!({"trace_id": "abc"})
    );
}

#[sqlx::test]
async fn read_with_empty_items_query_returns_nothing(pool: PgPool) {
    let store = PostgresEventStore::new(pool);
    store
        .append(
            vec![event("created", vec![]), event("updated", vec![])],
            None,
        )
        .await
        .unwrap();

    let result = read_all(&store, Query::from_items([]), None).await;

    assert!(result.is_empty());
}

#[sqlx::test]
async fn append_condition_with_empty_items_query_succeeds(pool: PgPool) {
    let store = PostgresEventStore::new(pool);
    store
        .append(vec![event("created", vec![])], None)
        .await
        .unwrap();

    let result = store
        .append(
            vec![event("updated", vec![])],
            Some(AppendCondition::new(Query::from_items([]), None)),
        )
        .await;

    assert!(result.is_ok());
}

#[sqlx::test]
async fn read_streams_across_batch_boundaries(pool: PgPool) {
    let store = PostgresEventStore::new(pool);
    let events: Vec<_> = (0..600)
        .map(|i| event(&format!("e{i}"), vec!["batch:test".into()]))
        .collect();

    store.append(events, None).await.unwrap();

    let result = read_all(&store, Query::all(), None).await;

    assert_eq!(result.len(), 600);
    for (i, sequenced) in result.iter().enumerate() {
        assert_eq!(sequenced.position, i as u64);
        assert_eq!(sequenced.event.event_type, format!("e{i}"));
    }
}
