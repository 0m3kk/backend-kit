use async_trait::async_trait;
use cqrs::{CatchupWorker, CheckpointStore, KvCheckpointStore, View, ViewError};
use event_sourcing::memory::InMemoryEventStore;
use event_sourcing::{
    DomainEvent, EventId, EventStore, EventType, Query, QueryItem, SequencedEvent,
};
use kv_store::memory::MemoryKvStore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserRegistered {
    pub user_id: String,
    pub username: String,
}

impl DomainEvent for UserRegistered {
    fn event_type() -> EventType {
        EventType::new("UserRegistered")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderPlaced {
    pub order_id: String,
    pub amount: u64,
}

impl DomainEvent for OrderPlaced {
    fn event_type() -> EventType {
        EventType::new("OrderPlaced")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserProfileView {
    pub user_id: String,
    pub username: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderSummaryView {
    pub order_id: String,
    pub amount: u64,
}

#[derive(Default, Clone)]
pub struct WorkerTestDb {
    pub users: Arc<RwLock<HashMap<String, UserProfileView>>>,
    pub orders: Arc<RwLock<HashMap<String, OrderSummaryView>>>,
}

#[async_trait]
impl View<WorkerTestDb> for UserProfileView {
    fn view_name(&self) -> &'static str {
        "user_profiles"
    }

    async fn apply_event(
        &self,
        event: &SequencedEvent,
        db: &WorkerTestDb,
    ) -> Result<(), ViewError> {
        if event.event.event_type == UserRegistered::event_type() {
            let payload: UserRegistered = serde_json::from_value(event.event.data.clone())
                .map_err(|e| ViewError::Deserialization(e.to_string()))?;

            let view = UserProfileView {
                user_id: payload.user_id.clone(),
                username: payload.username,
            };

            if let Ok(mut guard) = db.users.write() {
                guard.insert(payload.user_id, view);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl View<WorkerTestDb> for OrderSummaryView {
    fn view_name(&self) -> &'static str {
        "order_summaries"
    }

    async fn apply_event(
        &self,
        event: &SequencedEvent,
        db: &WorkerTestDb,
    ) -> Result<(), ViewError> {
        if event.event.event_type == OrderPlaced::event_type() {
            let payload: OrderPlaced = serde_json::from_value(event.event.data.clone())
                .map_err(|e| ViewError::Deserialization(e.to_string()))?;

            let view = OrderSummaryView {
                order_id: payload.order_id.clone(),
                amount: payload.amount,
            };

            if let Ok(mut guard) = db.orders.write() {
                guard.insert(payload.order_id, view);
            }
        }
        Ok(())
    }
}

pub struct UserOnlyView;

#[async_trait]
impl View<WorkerTestDb> for UserOnlyView {
    fn view_name(&self) -> &'static str {
        "user_only_view"
    }

    fn subscription_query(&self) -> Query {
        Query::item(QueryItem::new().with_type(UserRegistered::event_type()))
    }

    async fn apply_event(
        &self,
        event: &SequencedEvent,
        db: &WorkerTestDb,
    ) -> Result<(), ViewError> {
        let payload: UserRegistered = serde_json::from_value(event.event.data.clone())
            .map_err(|e| ViewError::Deserialization(e.to_string()))?;

        let view = UserProfileView {
            user_id: payload.user_id.clone(),
            username: payload.username,
        };

        if let Ok(mut guard) = db.users.write() {
            guard.insert(payload.user_id, view);
        }
        Ok(())
    }
}

pub struct ResilientTestView;

#[async_trait]
impl View<WorkerTestDb> for ResilientTestView {
    fn view_name(&self) -> &'static str {
        "resilient_view"
    }

    async fn apply_event(
        &self,
        event: &SequencedEvent,
        db: &WorkerTestDb,
    ) -> Result<(), ViewError> {
        if event.event.event_type == UserRegistered::event_type() {
            let payload: UserRegistered = serde_json::from_value(event.event.data.clone())
                .map_err(|e| ViewError::Deserialization(e.to_string()))?;

            if payload.user_id == "corrupt_user" {
                return Err(ViewError::Execution(
                    "Corrupt payload encountered".to_string(),
                ));
            }

            let view = UserProfileView {
                user_id: payload.user_id.clone(),
                username: payload.username,
            };

            if let Ok(mut guard) = db.users.write() {
                guard.insert(payload.user_id, view);
            }
        }
        Ok(())
    }
}

#[tokio::test]
async fn test_catchup_worker_multiple_views_parallel() {
    let event_store = Arc::new(InMemoryEventStore::new());
    let db = WorkerTestDb::default();
    let kv_store = MemoryKvStore::new();
    let checkpoint_store = KvCheckpointStore::new(kv_store);

    let user_event = UserRegistered {
        user_id: "user_1".to_string(),
        username: "Alice".to_string(),
    }
    .to_event(EventId::new("evt_1"))
    .unwrap();

    let order_event = OrderPlaced {
        order_id: "ord_100".to_string(),
        amount: 250,
    }
    .to_event(EventId::new("evt_2"))
    .unwrap();

    event_store
        .append(vec![user_event, order_event], None)
        .await
        .unwrap();

    let worker = CatchupWorker::new(db.clone(), event_store, checkpoint_store)
        .register_view(UserProfileView {
            user_id: String::new(),
            username: String::new(),
        })
        .register_view(OrderSummaryView {
            order_id: String::new(),
            amount: 0,
        });

    let total_processed = worker.catchup_all().await.unwrap();
    assert_eq!(total_processed, 4);

    assert_eq!(
        db.users.read().unwrap().get("user_1").cloned(),
        Some(UserProfileView {
            user_id: "user_1".to_string(),
            username: "Alice".to_string()
        })
    );

    assert_eq!(
        db.orders.read().unwrap().get("ord_100").cloned(),
        Some(OrderSummaryView {
            order_id: "ord_100".to_string(),
            amount: 250
        })
    );
}

#[tokio::test]
async fn test_worker_subscription_query_filtering() {
    let event_store = Arc::new(InMemoryEventStore::new());
    let db = WorkerTestDb::default();
    let kv_store = MemoryKvStore::new();
    let checkpoint_store = KvCheckpointStore::new(kv_store);

    let mut events = Vec::new();
    for i in 1..=2 {
        events.push(
            UserRegistered {
                user_id: format!("u_{i}"),
                username: format!("User {i}"),
            }
            .to_event(EventId::new(format!("u_evt_{i}")))
            .unwrap(),
        );
    }
    for i in 1..=3 {
        events.push(
            OrderPlaced {
                order_id: format!("o_{i}"),
                amount: i * 10,
            }
            .to_event(EventId::new(format!("o_evt_{i}")))
            .unwrap(),
        );
    }

    event_store.append(events, None).await.unwrap();

    let worker =
        CatchupWorker::new(db.clone(), event_store, checkpoint_store).register_view(UserOnlyView);

    let processed = worker.catchup_all().await.unwrap();
    assert_eq!(processed, 2);
    assert_eq!(db.users.read().unwrap().len(), 2);
}

#[tokio::test]
async fn test_worker_incremental_catchup_and_checkpoint_continuation() {
    let event_store = Arc::new(InMemoryEventStore::new());
    let db = WorkerTestDb::default();
    let kv_store = MemoryKvStore::new();
    let checkpoint_store = KvCheckpointStore::new(kv_store);

    let event1 = UserRegistered {
        user_id: "user_a".to_string(),
        username: "Alice".to_string(),
    }
    .to_event(EventId::new("evt_a"))
    .unwrap();
    let event2 = UserRegistered {
        user_id: "user_b".to_string(),
        username: "Bob".to_string(),
    }
    .to_event(EventId::new("evt_b"))
    .unwrap();

    event_store
        .append(vec![event1, event2], None)
        .await
        .unwrap();

    let worker = CatchupWorker::new(db.clone(), event_store.clone(), checkpoint_store)
        .register_view(UserProfileView {
            user_id: String::new(),
            username: String::new(),
        });

    let count1 = worker.catchup_all().await.unwrap();
    assert_eq!(count1, 2);

    let mut batch2 = Vec::new();
    for i in 1..=3 {
        batch2.push(
            UserRegistered {
                user_id: format!("user_b2_{i}"),
                username: format!("User B2 {i}"),
            }
            .to_event(EventId::new(format!("evt_b2_{i}")))
            .unwrap(),
        );
    }
    event_store.append(batch2, None).await.unwrap();

    let count2 = worker.catchup_all().await.unwrap();
    assert_eq!(count2, 3);
    assert_eq!(db.users.read().unwrap().len(), 5);
}

#[tokio::test]
async fn test_worker_error_resilience_and_checkpoint_preservation() {
    let event_store = Arc::new(InMemoryEventStore::new());
    let db = WorkerTestDb::default();
    let kv_store = MemoryKvStore::new();
    let checkpoint_store = KvCheckpointStore::new(kv_store.clone());

    let event1 = UserRegistered {
        user_id: "u_valid1".to_string(),
        username: "V1".to_string(),
    }
    .to_event(EventId::new("e1"))
    .unwrap();
    let event2 = UserRegistered {
        user_id: "u_valid2".to_string(),
        username: "V2".to_string(),
    }
    .to_event(EventId::new("e2"))
    .unwrap();
    let event3 = UserRegistered {
        user_id: "corrupt_user".to_string(),
        username: "ERR".to_string(),
    }
    .to_event(EventId::new("e3"))
    .unwrap();

    event_store
        .append(vec![event1, event2, event3], None)
        .await
        .unwrap();

    let worker = CatchupWorker::new(db.clone(), event_store, checkpoint_store)
        .register_view(ResilientTestView);

    let result = worker.catchup_all().await;
    assert!(result.is_err());

    let checkpoint = KvCheckpointStore::new(kv_store)
        .get_position(&db, "resilient_view")
        .await
        .unwrap();
    assert_eq!(checkpoint, Some(event_sourcing::SequencePosition::new(2)));
    assert_eq!(db.users.read().unwrap().len(), 2);
}
