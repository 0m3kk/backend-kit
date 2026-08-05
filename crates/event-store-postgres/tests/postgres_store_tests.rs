use event_sourcing::{
    AppendCondition, AppendError, Direction, Event, EventStore, Query, QueryItem, ReadOptions,
    SequencePosition, Tag,
};
use event_store_postgres::PostgresEventStore;
use futures_util::StreamExt;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

#[tokio::test]
async fn test_postgres_store_builder_options() {
    let mock_pool_res =
        PgPoolOptions::new().connect_lazy("postgres://postgres:postgres@localhost:5432/test_db");

    assert!(mock_pool_res.is_ok());
    let pool = mock_pool_res.unwrap();

    let store = PostgresEventStore::new(pool.clone())
        .with_chunk_size(250)
        .with_max_append_attempts(3);

    assert_eq!(
        store.pool().options().get_max_connections(),
        pool.options().get_max_connections()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_append_empty_batch_returns_error(pool: PgPool) {
    let store = PostgresEventStore::new(pool);
    let empty_res = store.append(vec![], None).await;
    assert!(matches!(empty_res, Err(AppendError::EmptyBatch)));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_append_single_and_batch_events(pool: PgPool) {
    let store = PostgresEventStore::new(pool).with_chunk_size(2);

    // Single event
    let event1 = Event::new(
        "ev-pg-1",
        "UserRegistered",
        serde_json::json!({"user_id": 42}),
        vec![Tag::key_value("user", "42")],
    );

    let appended1 = store
        .append(vec![event1], None)
        .await
        .expect("Append single failed");

    assert_eq!(appended1.len(), 1);
    assert_eq!(appended1[0].event.id.as_str(), "ev-pg-1");
    assert_eq!(appended1[0].position.value(), 1);

    // Batch events (Chunked)
    let batch = vec![
        Event::new(
            "ev-pg-2",
            "OrderCreated",
            serde_json::json!({"order_id": 100}),
            vec![Tag::key_value("user", "42")],
        ),
        Event::new(
            "ev-pg-3",
            "OrderCreated",
            serde_json::json!({"order_id": 101}),
            vec![Tag::key_value("user", "43")],
        ),
    ];

    let appended_batch = store
        .append(batch, None)
        .await
        .expect("Append batch failed");

    assert_eq!(appended_batch.len(), 2);
    assert_eq!(appended_batch[0].position.value(), 2);
    assert_eq!(appended_batch[1].position.value(), 3);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_read_all_events_stream(pool: PgPool) {
    let store = PostgresEventStore::new(pool);

    let events = vec![
        Event::new("ev-1", "TypeA", serde_json::json!({}), vec![]),
        Event::new("ev-2", "TypeB", serde_json::json!({}), vec![]),
    ];

    store.append(events, None).await.unwrap();

    let mut stream = store.read(&Query::all(), ReadOptions::new()).await;
    let mut fetched = Vec::new();
    while let Some(res) = stream.next().await {
        fetched.push(res.unwrap());
    }

    assert_eq!(fetched.len(), 2);
    assert_eq!(fetched[0].event.id.as_str(), "ev-1");
    assert_eq!(fetched[1].event.id.as_str(), "ev-2");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_read_options_limit_after_and_direction(pool: PgPool) {
    let store = PostgresEventStore::new(pool);

    let events = vec![
        Event::new("ev-1", "TypeA", serde_json::json!({}), vec![]),
        Event::new("ev-2", "TypeA", serde_json::json!({}), vec![]),
        Event::new("ev-3", "TypeA", serde_json::json!({}), vec![]),
        Event::new("ev-4", "TypeA", serde_json::json!({}), vec![]),
    ];

    store.append(events, None).await.unwrap();

    let opts = ReadOptions::new()
        .after(SequencePosition::new(1))
        .limit(2)
        .direction(Direction::Backward);

    let mut stream = store.read(&Query::all(), opts).await;
    let mut fetched = Vec::new();
    while let Some(res) = stream.next().await {
        fetched.push(res.unwrap());
    }

    assert_eq!(fetched.len(), 2);
    assert_eq!(fetched[0].position.value(), 4);
    assert_eq!(fetched[1].position.value(), 3);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_read_filtered_by_event_type_and_tags(pool: PgPool) {
    let store = PostgresEventStore::new(pool);

    let events = vec![
        Event::new(
            "ev-1",
            "UserRegistered",
            serde_json::json!({}),
            vec![Tag::key_value("user", "42")],
        ),
        Event::new(
            "ev-2",
            "OrderCreated",
            serde_json::json!({}),
            vec![Tag::key_value("user", "42")],
        ),
        Event::new(
            "ev-3",
            "UserRegistered",
            serde_json::json!({}),
            vec![Tag::key_value("user", "43")],
        ),
    ];

    store.append(events, None).await.unwrap();

    let query = Query::item(QueryItem::new().with_tag("user:42"));
    let mut stream = store.read(&query, ReadOptions::new()).await;
    let mut fetched = Vec::new();
    while let Some(res) = stream.next().await {
        fetched.push(res.unwrap());
    }

    assert_eq!(fetched.len(), 2);
    assert_eq!(fetched[0].event.id.as_str(), "ev-1");
    assert_eq!(fetched[1].event.id.as_str(), "ev-2");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_read_query_or_logic_across_items(pool: PgPool) {
    let store = PostgresEventStore::new(pool);

    let events = vec![
        Event::new("ev-1", "UserRegistered", serde_json::json!({}), vec![]),
        Event::new(
            "ev-2",
            "OrderCreated",
            serde_json::json!({}),
            vec![Tag::key_value("order", "100")],
        ),
        Event::new("ev-3", "OtherEvent", serde_json::json!({}), vec![]),
    ];

    store.append(events, None).await.unwrap();

    let multi_query = Query::from_items(vec![
        QueryItem::new().with_type("UserRegistered"),
        QueryItem::new().with_tag("order:100"),
    ]);

    let mut stream = store.read(&multi_query, ReadOptions::new()).await;
    let mut fetched = Vec::new();
    while let Some(res) = stream.next().await {
        fetched.push(res.unwrap());
    }

    assert_eq!(fetched.len(), 2);
    assert_eq!(fetched[0].event.id.as_str(), "ev-1");
    assert_eq!(fetched[1].event.id.as_str(), "ev-2");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_append_condition_success(pool: PgPool) {
    let store = PostgresEventStore::new(pool);

    let initial = vec![Event::new("ev-1", "TypeA", serde_json::json!({}), vec![])];
    let appended = store.append(initial, None).await.unwrap();

    let new_event = Event::new("ev-2", "TypeB", serde_json::json!({}), vec![]);
    let cond = AppendCondition::new(Query::item(QueryItem::new().with_type("TypeB")))
        .after(appended[0].position);

    let res = store.append(vec![new_event], Some(cond)).await;
    assert!(res.is_ok());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_append_condition_conflict_with_after(pool: PgPool) {
    let store = PostgresEventStore::new(pool);

    let initial = vec![
        Event::new("ev-1", "TypeA", serde_json::json!({}), vec![]),
        Event::new("ev-2", "TypeB", serde_json::json!({}), vec![]),
    ];
    let appended = store.append(initial, None).await.unwrap();

    let conflicting = Event::new("ev-3", "TypeB", serde_json::json!({}), vec![]);
    let conflict_cond = AppendCondition::new(Query::item(QueryItem::new().with_type("TypeB")))
        .after(appended[0].position); // ev-2 was appended at pos 2 > pos 1

    let res = store.append(vec![conflicting], Some(conflict_cond)).await;
    assert!(res.is_err());

    if let Err(AppendError::Conflict {
        condition: _,
        conflicting_event,
    }) = res
    {
        assert_eq!(conflicting_event.event.id.as_str(), "ev-2");
    } else {
        panic!("Expected AppendError::Conflict");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_append_condition_conflict_without_after(pool: PgPool) {
    let store = PostgresEventStore::new(pool);

    let initial = vec![Event::new("ev-1", "TypeA", serde_json::json!({}), vec![])];
    store.append(initial, None).await.unwrap();

    let conflicting = Event::new("ev-2", "TypeA", serde_json::json!({}), vec![]);
    let global_cond = AppendCondition::new(Query::item(QueryItem::new().with_type("TypeA")));

    let res = store.append(vec![conflicting], Some(global_cond)).await;
    assert!(res.is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_stream_spawn_tokio_task(pool: PgPool) {
    let store = PostgresEventStore::new(pool);

    let events = vec![
        Event::new("ev-1", "TypeA", serde_json::json!({}), vec![]),
        Event::new("ev-2", "TypeB", serde_json::json!({}), vec![]),
    ];

    store.append(events, None).await.unwrap();

    let store_clone = store.clone();
    let handle = tokio::spawn(async move {
        let mut stream = store_clone.read(&Query::all(), ReadOptions::new()).await;
        let mut count = 0;
        while let Some(res) = stream.next().await {
            assert!(res.is_ok());
            count += 1;
        }
        count
    });

    let spawned_count = handle.await.expect("Tokio task failed");
    assert_eq!(spawned_count, 2);
}
