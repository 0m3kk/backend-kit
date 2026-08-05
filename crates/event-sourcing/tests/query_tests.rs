#![allow(clippy::expect_used, clippy::unwrap_used)]

use event_sourcing::*;

#[test]
fn test_query_item_type_matching() {
    let event = Event::new(
        "ev-1",
        "UserRegistered",
        serde_json::json!({"username": "alice"}),
        vec![Tag::key_value("user", "123")],
    );

    let item_match = QueryItem::new().with_type("UserRegistered");
    assert!(item_match.matches(&event));

    let item_mismatch = QueryItem::new().with_type("UserDeleted");
    assert!(!item_mismatch.matches(&event));

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

    let q1 = QueryItem::new().with_tag("customer:cust-1");
    assert!(q1.matches(&event));

    let q_all_tags = QueryItem::new()
        .with_tag("customer:cust-1")
        .with_tag("merchant:merch-5")
        .with_tag("priority");
    assert!(q_all_tags.matches(&event));

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

    let q_valid = QueryItem::new()
        .with_type("PaymentProcessed")
        .with_tag("account:acc-42")
        .with_tag("usd");
    assert!(q_valid.matches(&event));

    let q_wrong_type = QueryItem::new()
        .with_type("PaymentFailed")
        .with_tag("account:acc-42");
    assert!(!q_wrong_type.matches(&event));

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

    let query = Query::from_items(vec![
        QueryItem::new().with_type("UserRegistered"),
        QueryItem::new().with_type("PaymentReceived"),
    ]);

    assert!(query.matches(&event_reg));
    assert!(query.matches(&event_pay));
    assert!(!query.matches(&event_unrelated));

    let query_all = Query::all();
    assert!(query_all.matches(&event_reg));
    assert!(query_all.matches(&event_pay));
    assert!(query_all.matches(&event_unrelated));
}

#[test]
fn test_query_fingerprint_sha256_format() {
    let q_all = Query::all();
    let fp = q_all.fingerprint();
    assert_eq!(fp.len(), 64);
    assert!(
        fp.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
    );
}

#[test]
fn test_query_fingerprint_uniqueness_different_queries() {
    let q_all = Query::all();
    let q_type_a = Query::item(QueryItem::new().with_type("TypeA"));
    let q_type_b = Query::item(QueryItem::new().with_type("TypeB"));
    let q_tag_1 = Query::item(QueryItem::new().with_tag("user:1"));
    let q_tag_2 = Query::item(QueryItem::new().with_tag("user:2"));
    let q_multi = Query::from_items(vec![
        QueryItem::new().with_type("TypeA"),
        QueryItem::new().with_tag("user:1"),
    ]);

    let fps = vec![
        q_all.fingerprint(),
        q_type_a.fingerprint(),
        q_type_b.fingerprint(),
        q_tag_1.fingerprint(),
        q_tag_2.fingerprint(),
        q_multi.fingerprint(),
    ];

    // Ensure all fingerprints are unique
    let mut unique_fps = fps.clone();
    unique_fps.sort();
    unique_fps.dedup();
    assert_eq!(fps.len(), unique_fps.len());
}

#[test]
fn test_query_fingerprint_order_independence() {
    let q_all = Query::all();
    assert_eq!(q_all.canonical_string(), "ALL");
    assert!(!q_all.fingerprint().is_empty());

    // Permutation 1: types=["TypeB", "TypeA"], tags=["user:42", "order:100"]
    let item1 = QueryItem::new()
        .with_type("TypeB")
        .with_type("TypeA")
        .with_tag("user:42")
        .with_tag("order:100");

    // Permutation 2: types=["TypeA", "TypeB"], tags=["order:100", "user:42"]
    let item2 = QueryItem::new()
        .with_type("TypeA")
        .with_type("TypeB")
        .with_tag("order:100")
        .with_tag("user:42");

    let query1 = Query::item(item1);
    let query2 = Query::item(item2);

    assert_eq!(query1.canonical_string(), query2.canonical_string());
    assert_eq!(query1.fingerprint(), query2.fingerprint());

    // Permutation across QueryItems
    let item_a = QueryItem::new().with_type("Alpha");
    let item_b = QueryItem::new().with_type("Beta");

    let q_ab = Query::from_items(vec![item_a.clone(), item_b.clone()]);
    let q_ba = Query::from_items(vec![item_b, item_a]);

    assert_eq!(q_ab.canonical_string(), q_ba.canonical_string());
    assert_eq!(q_ab.fingerprint(), q_ba.fingerprint());
}

#[test]
fn test_query_fingerprint_empty_query_equivalence() {
    let q_all = Query::all();
    let q_empty_items = Query::from_items(vec![]);

    assert_eq!(q_all.canonical_string(), q_empty_items.canonical_string());
    assert_eq!(q_all.fingerprint(), q_empty_items.fingerprint());
}

#[test]
fn test_query_combine_basic() {
    let q1 = Query::item(QueryItem::new().with_type("UserRegistered"));
    let q2 = Query::item(QueryItem::new().with_type("PaymentReceived"));

    let combined = Query::combine(vec![q1, q2]);
    assert_eq!(
        combined,
        Query::from_items(vec![
            QueryItem::new().with_type("PaymentReceived"),
            QueryItem::new().with_type("UserRegistered"),
        ])
    );
}

#[test]
fn test_query_combine_with_all_short_circuits() {
    let q_type = Query::item(QueryItem::new().with_type("OrderCreated"));
    let q_tag = Query::item(QueryItem::new().with_tag("user:42"));

    assert_eq!(Query::combine(vec![q_type.clone(), Query::All]), Query::All);
    assert_eq!(Query::combine(vec![Query::All, q_tag.clone()]), Query::All);
    assert_eq!(Query::combine(vec![q_type, Query::All, q_tag]), Query::All);
}

#[test]
fn test_query_combine_empty_query_item_collapses_to_all() {
    let empty_item_query = Query::item(QueryItem::new());
    let specific_query = Query::item(QueryItem::new().with_type("UserRegistered"));

    assert_eq!(
        Query::combine(vec![empty_item_query, specific_query]),
        Query::All
    );
}

#[test]
fn test_query_combine_deduplication_and_sorting() {
    let item_a1 = QueryItem::new().with_type("TypeA").with_tag("tag:1");
    let item_a2 = QueryItem::new().with_tag("tag:1").with_type("TypeA");
    let item_b = QueryItem::new().with_type("TypeB");

    let q1 = Query::item(item_a1);
    let q2 = Query::item(item_a2);
    let q3 = Query::item(item_b);

    let combined = Query::combine(vec![q1, q2, q3]);

    let expected = Query::from_items(vec![
        QueryItem::new().with_type("TypeA").with_tag("tag:1"),
        QueryItem::new().with_type("TypeB"),
    ]);

    assert_eq!(combined, expected);
}

#[test]
fn test_query_combine_multiple_decision_models_subsumption() {
    let decision_model_1_query = Query::item(QueryItem::new().with_tag("user:100"));
    let decision_model_2_query = Query::item(
        QueryItem::new()
            .with_type("OrderCreated")
            .with_tag("user:100"),
    );

    let combined = Query::combine(vec![decision_model_1_query, decision_model_2_query]);

    assert_eq!(combined, Query::item(QueryItem::new().with_tag("user:100")));
}

#[test]
fn test_query_combine_empty_iterator_returns_all() {
    let empty_combine = Query::combine(Vec::<Query>::new());
    assert_eq!(empty_combine, Query::All);
}

#[test]
fn test_query_item_subsumes_matrix() {
    // 1. Broad types subsumes narrow types
    let item_broad_types = QueryItem::new().with_type("TypeA").with_type("TypeB");
    let item_narrow_type = QueryItem::new().with_type("TypeA");
    assert!(item_broad_types.subsumes(&item_narrow_type));
    assert!(!item_narrow_type.subsumes(&item_broad_types));

    // 2. Broad tags subsumes narrow tags
    let item_broad_tags = QueryItem::new().with_tag("tag:1");
    let item_narrow_tags = QueryItem::new().with_tag("tag:1").with_tag("tag:2");
    assert!(item_broad_tags.subsumes(&item_narrow_tags));
    assert!(!item_narrow_tags.subsumes(&item_broad_tags));

    // 3. Empty types/tags subsumes specific types/tags
    let item_empty = QueryItem::new();
    let item_specific = QueryItem::new().with_type("TypeA").with_tag("tag:1");
    assert!(item_empty.subsumes(&item_specific));
    assert!(!item_specific.subsumes(&item_empty));

    // 4. Disjoint types/tags do not subsume each other
    let item_type_a = QueryItem::new().with_type("TypeA");
    let item_type_b = QueryItem::new().with_type("TypeB");
    assert!(!item_type_a.subsumes(&item_type_b));
    assert!(!item_type_b.subsumes(&item_type_a));

    let item_tag_1 = QueryItem::new().with_tag("tag:1");
    let item_tag_2 = QueryItem::new().with_tag("tag:2");
    assert!(!item_tag_1.subsumes(&item_tag_2));
    assert!(!item_tag_2.subsumes(&item_tag_1));
}
