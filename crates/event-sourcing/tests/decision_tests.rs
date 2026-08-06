use event_sourcing::memory::InMemoryEventStore;
use event_sourcing::{
    DecisionModel, Event, EventStore, EventStoreExt, Query, QueryItem, SequencePosition, Tag,
};

#[derive(Default, Debug, PartialEq, Eq)]
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

    fn is_overdrawn(&self) -> bool {
        self.balance < 0
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
async fn test_empty_decision_model_hydration() {
    let store = InMemoryEventStore::new();

    let model = store
        .load_decision_model(BankAccountModel::new("ACC-001"))
        .await
        .unwrap();

    assert_eq!(model.balance, 0);
    assert_eq!(model.account_id, "ACC-001");
    assert_eq!(model.last_position, None);
    assert!(!model.is_overdrawn());
}

#[tokio::test]
async fn test_decision_model_hydration_tracks_highest_position() {
    let store = InMemoryEventStore::new();

    let deposit_1 = Event::new(
        "evt-1",
        "MoneyDeposited",
        serde_json::json!({ "amount": 100 }),
        vec![Tag::key_value("account", "ACC-123")],
    );
    let deposit_2 = Event::new(
        "evt-2",
        "MoneyDeposited",
        serde_json::json!({ "amount": 50 }),
        vec![Tag::key_value("account", "ACC-123")],
    );
    let withdraw = Event::new(
        "evt-3",
        "MoneyWithdrawn",
        serde_json::json!({ "amount": 30 }),
        vec![Tag::key_value("account", "ACC-123")],
    );

    store
        .append(vec![deposit_1, deposit_2, withdraw], None)
        .await
        .unwrap();

    let loaded = store
        .load_decision_model(BankAccountModel::new("ACC-123"))
        .await
        .unwrap();

    assert_eq!(loaded.balance, 120);
    assert!(!loaded.is_overdrawn());
    assert_eq!(loaded.last_position, Some(SequencePosition::new(3)));
}

#[tokio::test]
async fn test_decision_model_filters_irrelevant_events() {
    let store = InMemoryEventStore::new();

    let evt1 = Event::new(
        "evt-1",
        "MoneyDeposited",
        serde_json::json!({ "amount": 100 }),
        vec![Tag::key_value("account", "ACC-123")],
    );
    let evt2 = Event::new(
        "evt-2",
        "MoneyDeposited",
        serde_json::json!({ "amount": 500 }),
        vec![Tag::key_value("account", "ACC-456")],
    );

    store.append(vec![evt1, evt2], None).await.unwrap();

    let acc123 = store
        .load_decision_model(BankAccountModel::new("ACC-123"))
        .await
        .unwrap();
    let acc456 = store
        .load_decision_model(BankAccountModel::new("ACC-456"))
        .await
        .unwrap();

    assert_eq!(acc123.balance, 100);
    assert_eq!(acc123.last_position, Some(SequencePosition::new(1)));

    assert_eq!(acc456.balance, 500);
    assert_eq!(acc456.last_position, Some(SequencePosition::new(2)));
}
