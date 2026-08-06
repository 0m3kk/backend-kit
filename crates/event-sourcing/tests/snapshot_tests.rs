use event_sourcing::memory::InMemoryEventStore;
use event_sourcing::{
    DecisionModel, Event, EventStore, EventStoreSnapshotExt, Query, QueryItem, SNAPSHOT_PREFIX,
    SequencePosition, SnapshotOptions, Tag, snapshot_key,
};
use kv_store::KvStore;
use kv_store::memory::MemoryKvStore;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct BankAccountModel {
    account_id: String,
    balance: i64,
}

impl BankAccountModel {
    fn new(account_id: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            balance: 0,
        }
    }
}

impl DecisionModel for BankAccountModel {
    fn query(&self) -> Query {
        Query::item(
            QueryItem::new()
                .with_types(["MoneyDeposited", "MoneyWithdrawn"])
                .with_tag(Tag::key_value("account", &self.account_id)),
        )
    }

    fn apply_event(&mut self, event: &Event) {
        match event.event_type.as_str() {
            "MoneyDeposited" => {
                let amount: i64 = serde_json::from_value(event.data["amount"].clone()).unwrap_or(0);
                self.balance += amount;
            }
            "MoneyWithdrawn" => {
                let amount: i64 = serde_json::from_value(event.data["amount"].clone()).unwrap_or(0);
                self.balance -= amount;
            }
            _ => {}
        }
    }
}

#[tokio::test]
async fn test_snapshot_key_format() {
    let model = BankAccountModel::new("ACC-999");
    let query = model.query();
    let key = snapshot_key(&query);

    let expected_key_prefix = format!("{}:", SNAPSHOT_PREFIX);
    assert!(key.to_string().starts_with(&expected_key_prefix));
    assert_eq!(
        key.to_string(),
        format!("{}:{}", SNAPSHOT_PREFIX, query.fingerprint())
    );
}

#[tokio::test]
async fn test_decision_model_snapshot_loading_and_auto_save() {
    let event_store = InMemoryEventStore::new();
    let kv_store = MemoryKvStore::new();

    // 1. Append initial 5 events
    let mut events = Vec::new();
    for i in 1..=5 {
        events.push(Event::new(
            format!("evt-{i}"),
            "MoneyDeposited",
            serde_json::json!({ "amount": 10 }),
            vec![Tag::key_value("account", "ACC-100")],
        ));
    }
    event_store.append(events, None).await.unwrap();

    // 2. Load with snapshot (threshold = 3 events -> triggers auto-save because 5 >= 3)
    let loaded = event_store
        .load_decision_model_with_snapshot(
            &kv_store,
            BankAccountModel::new("ACC-100"),
            SnapshotOptions::new(3),
        )
        .await
        .unwrap();

    assert_eq!(loaded.balance, 50);
    assert_eq!(loaded.last_position, Some(SequencePosition::new(5)));

    // Verify snapshot exists in KV store
    let model_query = BankAccountModel::new("ACC-100").query();
    let key = snapshot_key(&model_query);
    assert!(kv_store.exists(&key).await.unwrap());

    // 3. Append 3 more events (positions 6, 7, 8)
    let mut new_events = Vec::new();
    for i in 6..=8 {
        new_events.push(Event::new(
            format!("evt-{i}"),
            "MoneyDeposited",
            serde_json::json!({ "amount": 10 }),
            vec![Tag::key_value("account", "ACC-100")],
        ));
    }
    event_store.append(new_events, None).await.unwrap();

    // 4. Load again with snapshot (threshold = 2 -> 3 new events >= 2, catches up from position 5 to 8)
    let loaded_again = event_store
        .load_decision_model_with_snapshot(
            &kv_store,
            BankAccountModel::new("ACC-100"),
            SnapshotOptions::new(2),
        )
        .await
        .unwrap();

    assert_eq!(loaded_again.balance, 80);
    assert_eq!(loaded_again.last_position, Some(SequencePosition::new(8)));
}

#[tokio::test]
async fn test_snapshot_threshold_prevents_unnecessary_writes() {
    let event_store = InMemoryEventStore::new();
    let kv_store = MemoryKvStore::new();

    // Append 2 events
    let evt1 = Event::new(
        "evt-1",
        "MoneyDeposited",
        serde_json::json!({ "amount": 100 }),
        vec![Tag::key_value("account", "ACC-200")],
    );
    let evt2 = Event::new(
        "evt-2",
        "MoneyDeposited",
        serde_json::json!({ "amount": 50 }),
        vec![Tag::key_value("account", "ACC-200")],
    );
    event_store.append(vec![evt1, evt2], None).await.unwrap();

    // First load with high threshold = 10 (2 new events < 10 threshold)
    let loaded = event_store
        .load_decision_model_with_snapshot(
            &kv_store,
            BankAccountModel::new("ACC-200"),
            SnapshotOptions::new(10),
        )
        .await
        .unwrap();

    assert_eq!(loaded.balance, 150);

    // Snapshot key should NOT exist in KV store because 2 < 10
    let key = snapshot_key(&BankAccountModel::new("ACC-200").query());
    assert!(!kv_store.exists(&key).await.unwrap());
}

#[tokio::test]
async fn test_manual_save_snapshot() {
    let event_store = InMemoryEventStore::new();
    let kv_store = MemoryKvStore::new();

    let evt = Event::new(
        "evt-1",
        "MoneyDeposited",
        serde_json::json!({ "amount": 250 }),
        vec![Tag::key_value("account", "ACC-300")],
    );
    event_store.append(vec![evt], None).await.unwrap();

    // Load model without auto-saving snapshot (threshold = 100)
    let loaded = event_store
        .load_decision_model_with_snapshot(
            &kv_store,
            BankAccountModel::new("ACC-300"),
            SnapshotOptions::new(100),
        )
        .await
        .unwrap();

    assert_eq!(loaded.balance, 250);

    // Explicitly call save_snapshot
    loaded.save_snapshot(&kv_store).await.unwrap();

    // Verify key exists now
    let key = snapshot_key(&BankAccountModel::new("ACC-300").query());
    assert!(kv_store.exists(&key).await.unwrap());
}
