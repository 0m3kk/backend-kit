use event_sourcing::*;

#[test]
fn test_event_type_and_id_creation() {
    let id = EventId::new("evt-100");
    assert_eq!(id.as_str(), "evt-100");
    assert_eq!(id.to_string(), "evt-100");

    let event_type = EventType::new("OrderCreated");
    assert_eq!(event_type.as_str(), "OrderCreated");
    assert_eq!(event_type.to_string(), "OrderCreated");

    let tag = Tag::key_value("order", "ord-1");
    assert_eq!(tag.as_str(), "order:ord-1");
}

#[test]
fn test_sequence_position_behavior() {
    let pos0 = SequencePosition::ZERO;
    assert_eq!(pos0.value(), 0);
    assert_eq!(pos0.to_string(), "0");

    let pos1 = pos0.next();
    assert_eq!(pos1.value(), 1);
    assert!(pos1 > pos0);
}

#[test]
fn test_query_item_type_matching() {
    let event = Event::new(
        "ev-1",
        "UserRegistered",
        serde_json::json!({"username": "alice"}),
        vec![Tag::key_value("user", "123")],
    );

    // Matching single type
    let item_match = QueryItem::new().with_type("UserRegistered");
    assert!(item_match.matches(&event));

    // Non-matching type
    let item_mismatch = QueryItem::new().with_type("UserDeleted");
    assert!(!item_mismatch.matches(&event));

    // Multiple types (OR logic): matches if event.event_type is in types
    let item_multi_types = QueryItem::new().with_types(vec!["UserLoggedIn", "UserRegistered"]);
    assert!(item_multi_types.matches(&event));

    let item_multi_types_miss = QueryItem::new().with_types(vec!["UserLoggedIn", "UserDeleted"]);
    assert!(!item_multi_types_miss.matches(&event));
}

#[test]
fn test_query_item_tag_matching() {
    let event = Event::new(
        "ev-2",
        "OrderPlaced",
        serde_json::json!({"order_id": "ord-99"}),
        vec![
            Tag::key_value("customer", "cust-1"),
            Tag::key_value("merchant", "merch-5"),
            Tag::new("priority"),
        ],
    );

    // Single tag match
    let q1 = QueryItem::new().with_tag("customer:cust-1");
    assert!(q1.matches(&event));

    // Multiple tags (AND logic): must contain ALL specified tags
    let q_all_tags = QueryItem::new()
        .with_tag("customer:cust-1")
        .with_tag("merchant:merch-5")
        .with_tag("priority");
    assert!(q_all_tags.matches(&event));

    // One missing tag causes mismatch
    let q_missing_tag = QueryItem::new()
        .with_tag("customer:cust-1")
        .with_tag("priority")
        .with_tag("express");
    assert!(!q_missing_tag.matches(&event));
}

#[test]
fn test_query_item_type_and_tag_combination() {
    let event = Event::new(
        "ev-3",
        "PaymentProcessed",
        serde_json::json!({"amount": 100}),
        vec![Tag::key_value("account", "acc-42"), Tag::new("usd")],
    );

    // Correct type AND correct tags
    let q_valid = QueryItem::new()
        .with_type("PaymentProcessed")
        .with_tag("account:acc-42")
        .with_tag("usd");
    assert!(q_valid.matches(&event));

    // Wrong type, correct tags => mismatch
    let q_wrong_type = QueryItem::new()
        .with_type("PaymentFailed")
        .with_tag("account:acc-42");
    assert!(!q_wrong_type.matches(&event));

    // Correct type, wrong tags => mismatch
    let q_wrong_tag = QueryItem::new()
        .with_type("PaymentProcessed")
        .with_tag("eur");
    assert!(!q_wrong_tag.matches(&event));
}

#[test]
fn test_query_or_logic_across_query_items() {
    let event_reg = Event::new(
        "ev-1",
        "UserRegistered",
        serde_json::json!({"user": "alice"}),
        vec![Tag::key_value("user", "u-1")],
    );
    let event_pay = Event::new(
        "ev-2",
        "PaymentReceived",
        serde_json::json!({"amount": 50}),
        vec![Tag::key_value("user", "u-1")],
    );
    let event_unrelated = Event::new(
        "ev-3",
        "SystemBooted",
        serde_json::json!({}),
        vec![Tag::new("system")],
    );

    // Query matching either UserRegistered OR PaymentReceived
    let query = Query::from_items(vec![
        QueryItem::new().with_type("UserRegistered"),
        QueryItem::new().with_type("PaymentReceived"),
    ]);

    assert!(query.matches(&event_reg));
    assert!(query.matches(&event_pay));
    assert!(!query.matches(&event_unrelated));

    // Query::all() matches everything
    let query_all = Query::all();
    assert!(query_all.matches(&event_reg));
    assert!(query_all.matches(&event_pay));
    assert!(query_all.matches(&event_unrelated));
}

#[test]
fn test_append_condition_evaluation() {
    let query = Query::item(QueryItem::new().with_tag("username:alice"));
    let condition = AppendCondition::new(query).after(SequencePosition::new(10));

    let matching_event = Event::new(
        "ev-1",
        "UsernameClaimed",
        serde_json::json!({"username": "alice"}),
        vec![Tag::key_value("username", "alice")],
    );

    let non_matching_event = Event::new(
        "ev-2",
        "UsernameClaimed",
        serde_json::json!({"username": "bob"}),
        vec![Tag::key_value("username", "bob")],
    );

    // Event at pos 5 (<= after 10) does NOT violate condition
    let seq_at_5 = SequencedEvent::new(SequencePosition::new(5), 1000, matching_event.clone());
    assert!(!condition.is_violated_by(&seq_at_5));

    // Event at pos 10 (<= after 10) does NOT violate condition
    let seq_at_10 = SequencedEvent::new(SequencePosition::new(10), 1000, matching_event.clone());
    assert!(!condition.is_violated_by(&seq_at_10));

    // Event at pos 11 (> after 10) with matching query DOES violate condition
    let seq_at_11 = SequencedEvent::new(SequencePosition::new(11), 1000, matching_event);
    assert!(condition.is_violated_by(&seq_at_11));

    // Event at pos 11 (> after 10) with non-matching query does NOT violate condition
    let seq_at_11_other = SequencedEvent::new(SequencePosition::new(11), 1000, non_matching_event);
    assert!(!condition.is_violated_by(&seq_at_11_other));
}

#[test]
fn test_read_options_builder() {
    let opts = ReadOptions::new()
        .after(SequencePosition::new(42))
        .limit(100)
        .direction(Direction::Backward);

    assert_eq!(opts.after, Some(SequencePosition::new(42)));
    assert_eq!(opts.limit, Some(100));
    assert_eq!(opts.direction, Direction::Backward);
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct OrderCreated {
    order_id: String,
    total_amount: f64,
}

impl DomainEvent for OrderCreated {
    fn event_type() -> EventType {
        EventType::new("OrderCreated")
    }

    fn tags(&self) -> Vec<Tag> {
        vec![Tag::key_value("order", &self.order_id)]
    }
}

#[test]
fn test_domain_event_trait_and_conversion() {
    let domain_evt = OrderCreated {
        order_id: "ORD-123".to_string(),
        total_amount: 199.99,
    };

    let event = domain_evt.to_event("evt-1").unwrap();
    assert_eq!(event.id.as_str(), "evt-1");
    assert_eq!(event.event_type.as_str(), "OrderCreated");
    assert_eq!(event.tags, vec![Tag::new("order:ORD-123")]);

    let restored: OrderCreated = event.to_domain_event().unwrap();
    assert_eq!(restored, domain_evt);

    let seq_evt = SequencedEvent::new(SequencePosition::new(1), 1000, event);
    let seq_restored: OrderCreated = seq_evt.to_domain_event().unwrap();
    assert_eq!(seq_restored, domain_evt);
}
