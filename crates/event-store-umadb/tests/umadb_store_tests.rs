#![allow(clippy::expect_used, clippy::unwrap_used)]

use event_sourcing::{
    AppendCondition, AppendError, Direction, Event, EventStore, Query, QueryItem, ReadOptions, Tag,
};
use event_store_umadb::UmaDBEventStore;
use futures_util::StreamExt;
use std::env;
use umadb_client::AsyncUmaDbClient;
use uuid::Uuid;

#[test]
fn test_umadb_store_client_accessor() {
    let mock_client = tokio::runtime::Runtime::new().unwrap().block_on(
        AsyncUmaDbClient::connect_with_tls_options(
            "http://127.0.0.1:50051".to_string(),
            None,
            Some(100),
            None,
        ),
    );

    if let Ok(client) = mock_client {
        let store = UmaDBEventStore::new(client);
        store.client().close();
    }
}

/// Helper function to retrieve UMADB_URL or fail with clear instruction
fn get_umadb_url() -> String {
    env::var("UMADB_URL").expect("UMADB_URL environment variable MUST be set to run umadb_store_tests (e.g. UMADB_URL=http://127.0.0.1:50051)")
}

#[tokio::test]
async fn test_live_umadb_append_empty_batch() {
    let url = get_umadb_url();

    let store = UmaDBEventStore::connect(url, None)
        .await
        .expect("Failed to connect to UmaDB");

    let empty_res = store.append(vec![], None).await;
    assert!(matches!(empty_res, Err(AppendError::EmptyBatch)));
}

#[tokio::test]
async fn test_live_umadb_append_single_and_batch_events() {
    let url = get_umadb_url();

    let store = UmaDBEventStore::connect(url, None)
        .await
        .expect("Failed to connect to UmaDB");

    let event1_id = Uuid::new_v4().to_string();
    let event1 = Event::new(
        &event1_id,
        "UserRegistered",
        serde_json::json!({"user_id": 42}),
        vec![Tag::key_value("user", "42")],
    );

    let appended1 = store
        .append(vec![event1], None)
        .await
        .expect("Append single event failed");

    assert_eq!(appended1.len(), 1);
    assert_eq!(appended1[0].event.id.as_str(), event1_id);

    let event2_id = Uuid::new_v4().to_string();
    let event3_id = Uuid::new_v4().to_string();
    let batch = vec![
        Event::new(
            &event2_id,
            "OrderCreated",
            serde_json::json!({"order_id": 100}),
            vec![Tag::key_value("user", "42")],
        ),
        Event::new(
            &event3_id,
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
}

#[tokio::test]
async fn test_live_umadb_read_all_stream() {
    let url = get_umadb_url();

    let store = UmaDBEventStore::connect(url, None)
        .await
        .expect("Failed to connect to UmaDB");

    let event1_id = Uuid::new_v4().to_string();
    let event1 = Event::new(&event1_id, "TypeA", serde_json::json!({}), vec![]);
    store.append(vec![event1], None).await.unwrap();

    let mut stream = store.read(&Query::all(), ReadOptions::new()).await;
    let mut count = 0;
    while let Some(res) = stream.next().await {
        assert!(res.is_ok());
        count += 1;
    }
    assert!(count >= 1);
}

#[tokio::test]
async fn test_live_umadb_read_options_limit_and_direction() {
    let url = get_umadb_url();

    let store = UmaDBEventStore::connect(url, None)
        .await
        .expect("Failed to connect to UmaDB");

    let tag = format!("session:{}", Uuid::new_v4());
    let events = vec![
        Event::new(
            Uuid::new_v4().to_string(),
            "TypeB",
            serde_json::json!({}),
            vec![Tag::new(&tag)],
        ),
        Event::new(
            Uuid::new_v4().to_string(),
            "TypeB",
            serde_json::json!({}),
            vec![Tag::new(&tag)],
        ),
    ];

    let appended = store.append(events, None).await.unwrap();

    let query = Query::item(QueryItem::new().with_tag(&tag));
    let opts = ReadOptions::new().limit(1).direction(Direction::Backward);

    let mut stream = store.read(&query, opts).await;
    let mut fetched = Vec::new();
    while let Some(res) = stream.next().await {
        fetched.push(res.unwrap());
    }

    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0].position, appended[1].position);
}

#[tokio::test]
async fn test_live_umadb_append_condition_success() {
    let url = get_umadb_url();

    let store = UmaDBEventStore::connect(url, None)
        .await
        .expect("Failed to connect to UmaDB");

    let tag = format!("unique_cond:{}", Uuid::new_v4());
    let event1 = Event::new(
        Uuid::new_v4().to_string(),
        "UniqueTypeA",
        serde_json::json!({}),
        vec![Tag::new(&tag)],
    );
    let appended = store.append(vec![event1], None).await.unwrap();

    let event2 = Event::new(
        Uuid::new_v4().to_string(),
        "UniqueTypeB",
        serde_json::json!({}),
        vec![Tag::new(&tag)],
    );

    let cond = AppendCondition::new(Query::item(QueryItem::new().with_type("UniqueTypeB")))
        .after(appended[0].position);

    let res = store.append(vec![event2], Some(cond)).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_live_umadb_append_condition_conflict() {
    let url = get_umadb_url();

    let store = UmaDBEventStore::connect(url, None)
        .await
        .expect("Failed to connect to UmaDB");

    let tag = format!("conflict_tag:{}", Uuid::new_v4());
    let event1 = Event::new(
        Uuid::new_v4().to_string(),
        "ConflictingType",
        serde_json::json!({}),
        vec![Tag::new(&tag)],
    );
    let _appended = store.append(vec![event1], None).await.unwrap();

    let event2 = Event::new(
        Uuid::new_v4().to_string(),
        "ConflictingType",
        serde_json::json!({}),
        vec![Tag::new(&tag)],
    );

    let conflict_cond =
        AppendCondition::new(Query::item(QueryItem::new().with_type("ConflictingType")));

    let res = store.append(vec![event2], Some(conflict_cond)).await;
    assert!(res.is_err());
    assert!(matches!(res, Err(AppendError::Conflict { .. })));
}

#[tokio::test]
async fn test_live_umadb_stream_tokio_spawn() {
    let url = get_umadb_url();

    let store = UmaDBEventStore::connect(url, None)
        .await
        .expect("Failed to connect to UmaDB");

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

    let count = handle.await.expect("Tokio spawn failed");
    assert!(count >= 0);
}
