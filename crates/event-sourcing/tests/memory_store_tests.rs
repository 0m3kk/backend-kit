use event_sourcing::memory::InMemoryEventStore;
use event_sourcing::*;
use futures_util::StreamExt;

#[tokio::test]
async fn test_empty_store() {
    let store = InMemoryEventStore::new();

    assert!(store.is_empty());
    assert_eq!(store.len(), 0);

    let query_all = Query::all();
    let mut stream = store.read(&query_all, ReadOptions::new()).await;
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn test_append_empty_batch_returns_error() {
    let store = InMemoryEventStore::new();
    let result = store.append(vec![], None).await;

    assert!(matches!(result, Err(AppendError::EmptyBatch)));
}

#[tokio::test]
async fn test_append_single_and_batch_assigns_positions_and_timestamps() {
    let store = InMemoryEventStore::new();

    let e1 = Event::new(
        "evt-1",
        "UserCreated",
        serde_json::json!({"name": "Alice"}),
        vec![Tag::key_value("user", "u-1")],
    );

    let appended1 = store.append(vec![e1], None).await.unwrap();
    assert_eq!(appended1.len(), 1);
    assert_eq!(appended1[0].position, SequencePosition::new(1));
    assert!(appended1[0].timestamp > 0);
    assert_eq!(store.len(), 1);

    let e2 = Event::new(
        "evt-2",
        "OrderPlaced",
        serde_json::json!({"amount": 100}),
        vec![Tag::key_value("order", "ord-1")],
    );
    let e3 = Event::new(
        "evt-3",
        "PaymentReceived",
        serde_json::json!({"amount": 100}),
        vec![Tag::key_value("order", "ord-1")],
    );

    let appended2 = store.append(vec![e2, e3], None).await.unwrap();
    assert_eq!(appended2.len(), 2);
    assert_eq!(appended2[0].position, SequencePosition::new(2));
    assert_eq!(appended2[1].position, SequencePosition::new(3));
    assert_eq!(store.len(), 3);
}

#[tokio::test]
async fn test_read_all_stream() {
    let store = InMemoryEventStore::new();

    let events = vec![
        Event::new("e1", "TypeA", serde_json::json!({}), vec![]),
        Event::new("e2", "TypeB", serde_json::json!({}), vec![]),
        Event::new("e3", "TypeC", serde_json::json!({}), vec![]),
    ];

    store.append(events, None).await.unwrap();

    let query_all = Query::all();
    let mut stream = store.read(&query_all, ReadOptions::new()).await;

    let mut collected = Vec::new();
    while let Some(res) = stream.next().await {
        collected.push(res.unwrap());
    }

    assert_eq!(collected.len(), 3);
    assert_eq!(collected[0].position, SequencePosition::new(1));
    assert_eq!(collected[1].position, SequencePosition::new(2));
    assert_eq!(collected[2].position, SequencePosition::new(3));
}

#[tokio::test]
async fn test_read_filtered_by_event_type() {
    let store = InMemoryEventStore::new();

    let events = vec![
        Event::new("e1", "OrderCreated", serde_json::json!({}), vec![]),
        Event::new("e2", "OrderPaid", serde_json::json!({}), vec![]),
        Event::new("e3", "OrderShipped", serde_json::json!({}), vec![]),
        Event::new("e4", "OrderPaid", serde_json::json!({}), vec![]),
    ];

    store.append(events, None).await.unwrap();

    let query = Query::item(QueryItem::new().with_type("OrderPaid"));
    let mut stream = store.read(&query, ReadOptions::new()).await;

    let mut paid_events = Vec::new();
    while let Some(res) = stream.next().await {
        paid_events.push(res.unwrap());
    }

    assert_eq!(paid_events.len(), 2);
    assert_eq!(paid_events[0].position, SequencePosition::new(2));
    assert_eq!(paid_events[1].position, SequencePosition::new(4));
}

#[tokio::test]
async fn test_read_filtered_by_tags() {
    let store = InMemoryEventStore::new();

    let events = vec![
        Event::new(
            "e1",
            "AccountOpened",
            serde_json::json!({}),
            vec![Tag::key_value("account", "acc-1")],
        ),
        Event::new(
            "e2",
            "AccountOpened",
            serde_json::json!({}),
            vec![Tag::key_value("account", "acc-2")],
        ),
        Event::new(
            "e3",
            "MoneyDeposited",
            serde_json::json!({}),
            vec![Tag::key_value("account", "acc-1")],
        ),
    ];

    store.append(events, None).await.unwrap();

    let query_acc1 = Query::item(QueryItem::new().with_tag("account:acc-1"));
    let mut stream = store.read(&query_acc1, ReadOptions::new()).await;

    let mut acc1_events = Vec::new();
    while let Some(res) = stream.next().await {
        acc1_events.push(res.unwrap());
    }

    assert_eq!(acc1_events.len(), 2);
    assert_eq!(acc1_events[0].event.id.as_str(), "e1");
    assert_eq!(acc1_events[1].event.id.as_str(), "e3");
}

#[tokio::test]
async fn test_read_options_after() {
    let store = InMemoryEventStore::new();

    let events = (1..=5)
        .map(|i| Event::new(format!("e{}", i), "Ping", serde_json::json!({}), vec![]))
        .collect();

    store.append(events, None).await.unwrap();

    let query_all = Query::all();
    let opts = ReadOptions::new().after(SequencePosition::new(3));
    let mut stream = store.read(&query_all, opts).await;

    let mut remaining = Vec::new();
    while let Some(res) = stream.next().await {
        remaining.push(res.unwrap());
    }

    assert_eq!(remaining.len(), 2);
    assert_eq!(remaining[0].position, SequencePosition::new(4));
    assert_eq!(remaining[1].position, SequencePosition::new(5));
}

#[tokio::test]
async fn test_read_options_limit() {
    let store = InMemoryEventStore::new();

    let events = (1..=10)
        .map(|i| {
            Event::new(
                format!("e{}", i),
                "ItemAdded",
                serde_json::json!({}),
                vec![],
            )
        })
        .collect();

    store.append(events, None).await.unwrap();

    let query_all = Query::all();
    let opts = ReadOptions::new().limit(4);
    let mut stream = store.read(&query_all, opts).await;

    let mut limited = Vec::new();
    while let Some(res) = stream.next().await {
        limited.push(res.unwrap());
    }

    assert_eq!(limited.len(), 4);
    assert_eq!(limited[3].position, SequencePosition::new(4));
}

#[tokio::test]
async fn test_read_options_backward_direction() {
    let store = InMemoryEventStore::new();

    let events = vec![
        Event::new("e1", "Step1", serde_json::json!({}), vec![]),
        Event::new("e2", "Step2", serde_json::json!({}), vec![]),
        Event::new("e3", "Step3", serde_json::json!({}), vec![]),
    ];

    store.append(events, None).await.unwrap();

    let query_all = Query::all();
    let opts = ReadOptions::new().direction(Direction::Backward);
    let mut stream = store.read(&query_all, opts).await;

    let mut reversed = Vec::new();
    while let Some(res) = stream.next().await {
        reversed.push(res.unwrap());
    }

    assert_eq!(reversed.len(), 3);
    assert_eq!(reversed[0].position, SequencePosition::new(3));
    assert_eq!(reversed[1].position, SequencePosition::new(2));
    assert_eq!(reversed[2].position, SequencePosition::new(1));
}

#[tokio::test]
async fn test_append_condition_success() {
    let store = InMemoryEventStore::new();

    let e1 = Event::new(
        "e1",
        "UsernameClaimed",
        serde_json::json!({"user": "john"}),
        vec![Tag::key_value("username", "john")],
    );
    let appended1 = store.append(vec![e1], None).await.unwrap();
    let pos1 = appended1[0].position;

    let query = Query::item(QueryItem::new().with_tag("username:john"));
    let condition = AppendCondition::new(query).after(pos1);

    let e2 = Event::new(
        "e2",
        "UserProfileUpdated",
        serde_json::json!({"bio": "hello"}),
        vec![Tag::key_value("user", "u-john")],
    );

    let res = store.append(vec![e2], Some(condition)).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_append_condition_conflict_failure() {
    let store = InMemoryEventStore::new();

    let e1 = Event::new(
        "e1",
        "UsernameClaimed",
        serde_json::json!({"user": "john"}),
        vec![Tag::key_value("username", "john")],
    );
    let appended1 = store.append(vec![e1], None).await.unwrap();
    let pos1 = appended1[0].position;

    let e2 = Event::new(
        "e2",
        "UsernameClaimed",
        serde_json::json!({"user": "john"}),
        vec![Tag::key_value("username", "john")],
    );
    store.append(vec![e2], None).await.unwrap();

    let query = Query::item(QueryItem::new().with_tag("username:john"));
    let condition = AppendCondition::new(query).after(pos1);

    let e3 = Event::new(
        "e3",
        "UsernameClaimed",
        serde_json::json!({"user": "john"}),
        vec![Tag::key_value("username", "john")],
    );

    let err = store.append(vec![e3], Some(condition)).await.unwrap_err();

    match err {
        AppendError::Conflict {
            conflicting_event, ..
        } => {
            assert_eq!(conflicting_event.position, SequencePosition::new(2));
            assert_eq!(conflicting_event.event.id.as_str(), "e2");
        }
        _ => panic!("Expected AppendError::Conflict"),
    }
}

#[tokio::test]
async fn test_append_condition_without_after_checks_all_events() {
    let store = InMemoryEventStore::new();

    let e1 = Event::new(
        "e1",
        "GlobalLockClaimed",
        serde_json::json!({}),
        vec![Tag::new("lock:resource-x")],
    );
    store.append(vec![e1], None).await.unwrap();

    let query = Query::item(QueryItem::new().with_tag("lock:resource-x"));
    let condition = AppendCondition::new(query);

    let e2 = Event::new(
        "e2",
        "GlobalLockClaimed",
        serde_json::json!({}),
        vec![Tag::new("lock:resource-x")],
    );

    let err = store.append(vec![e2], Some(condition)).await.unwrap_err();
    assert!(matches!(err, AppendError::Conflict { .. }));
}

#[tokio::test]
async fn test_multi_tag_partitioning_concurrency() {
    let store = InMemoryEventStore::new();

    let e_alice = Event::new(
        "e1",
        "UsernameClaimed",
        serde_json::json!({}),
        vec![Tag::key_value("username", "alice")],
    );
    let pos_alice = store.append(vec![e_alice], None).await.unwrap()[0].position;

    let query_bob = Query::item(QueryItem::new().with_tag("username:bob"));
    let condition_bob = AppendCondition::new(query_bob).after(pos_alice);

    let e_bob = Event::new(
        "e2",
        "UsernameClaimed",
        serde_json::json!({}),
        vec![Tag::key_value("username", "bob")],
    );

    assert!(store.append(vec![e_bob], Some(condition_bob)).await.is_ok());
}

#[tokio::test]
async fn test_stream_spawn_tokio_task() {
    let store = InMemoryEventStore::new();

    let e1 = Event::new("e1", "AsyncEvent", serde_json::json!({}), vec![]);
    store.append(vec![e1], None).await.unwrap();

    let query_all = Query::all();
    let mut stream = store.read(&query_all, ReadOptions::new()).await;

    let handle = tokio::spawn(async move {
        let mut count = 0;
        while let Some(res) = stream.next().await {
            res.unwrap();
            count += 1;
        }
        count
    });

    let count = handle.await.unwrap();
    assert_eq!(count, 1);
}
