use async_trait::async_trait;
use cqrs::{KvCheckpointStore, ReadConsistency, View, ViewError, ViewQueryEngine};
use event_sourcing::memory::InMemoryEventStore;
use event_sourcing::{DomainEvent, EventId, EventStore, EventType, Query, SequencedEvent};
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
pub struct UserProfileView {
    pub user_id: String,
    pub username: String,
}

#[derive(Default, Clone)]
pub struct ViewTestDb {
    pub users: Arc<RwLock<HashMap<String, UserProfileView>>>,
}

#[async_trait]
impl View<ViewTestDb> for UserProfileView {
    fn view_name(&self) -> &'static str {
        "user_profiles"
    }

    fn subscription_query(&self) -> Query {
        Query::all()
    }

    async fn apply_event(&self, event: &SequencedEvent, db: &ViewTestDb) -> Result<(), ViewError> {
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

#[tokio::test]
async fn test_view_projection_apply_event() {
    let db = ViewTestDb::default();
    let view = UserProfileView {
        user_id: String::new(),
        username: String::new(),
    };

    assert_eq!(view.view_name(), "user_profiles");

    let domain_evt = UserRegistered {
        user_id: "user_42".to_string(),
        username: "Douglas".to_string(),
    };
    let event = domain_evt.to_event(EventId::new("evt_42")).unwrap();
    let sequenced = SequencedEvent {
        position: event_sourcing::SequencePosition::new(1),
        event,
        timestamp: 1000,
    };

    view.apply_event(&sequenced, &db).await.unwrap();

    let user = db.users.read().unwrap().get("user_42").cloned();
    assert_eq!(
        user,
        Some(UserProfileView {
            user_id: "user_42".to_string(),
            username: "Douglas".to_string(),
        })
    );
}

#[tokio::test]
async fn test_query_engine_eventual_vs_strong_consistency() {
    let event_store = Arc::new(InMemoryEventStore::new());
    let db = ViewTestDb::default();
    let kv_store = MemoryKvStore::new();
    let checkpoint_store = KvCheckpointStore::new(kv_store);

    let event = UserRegistered {
        user_id: "user_strong".to_string(),
        username: "Strong".to_string(),
    }
    .to_event(EventId::new("evt_strong"))
    .unwrap();

    event_store.append(vec![event], None).await.unwrap();

    let query_engine = ViewQueryEngine::new(
        UserProfileView {
            user_id: String::new(),
            username: String::new(),
        },
        db,
        event_store,
        checkpoint_store,
    );

    // 1. Query with Eventual consistency (returns None, view not caught up yet)
    let eventual_res = query_engine
        .query(ReadConsistency::Eventual, |ctx| {
            Ok(ctx.users.read().unwrap().get("user_strong").cloned())
        })
        .await
        .unwrap();
    assert_eq!(eventual_res, None);

    // 2. Query with Strong consistency (triggers catchup, returns Some)
    let strong_res = query_engine
        .query(ReadConsistency::Strong, |ctx| {
            Ok(ctx.users.read().unwrap().get("user_strong").cloned())
        })
        .await
        .unwrap();
    assert_eq!(
        strong_res,
        Some(UserProfileView {
            user_id: "user_strong".to_string(),
            username: "Strong".to_string(),
        })
    );
}
