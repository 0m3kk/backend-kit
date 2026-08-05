#[cfg(test)]
mod tests {
    use crate::*;
    use uuid::Uuid;

    #[test]
    fn test_convert_event_with_uuid_and_metadata() {
        let test_uuid = Uuid::new_v4();
        let event = Event::new(
            test_uuid.to_string(),
            "OrderCreated",
            serde_json::json!({"total": 99.9}),
            vec![Tag::key_value("order", "100"), Tag::key_value("user", "42")],
        )
        .with_metadata(serde_json::json!({"client_ip": "127.0.0.1"}));

        let dcb_evt = convert_event(event.clone());
        assert_eq!(dcb_evt.event_type, "OrderCreated");
        assert_eq!(dcb_evt.uuid, Some(test_uuid));
        assert_eq!(dcb_evt.tags, vec!["order:100", "user:42"]);

        let converted_back = convert_dcb_event(dcb_evt);
        assert_eq!(converted_back.id.as_str(), test_uuid.to_string());
        assert_eq!(converted_back.event_type.as_str(), "OrderCreated");
        assert_eq!(converted_back.data["total"], 99.9);
        assert_eq!(converted_back.tags.len(), 2);
        assert_eq!(converted_back.metadata.unwrap()["client_ip"], "127.0.0.1");
    }

    #[test]
    fn test_convert_query_all_and_items() {
        let all_query = Query::all();
        let dcb_all = convert_query(&all_query);
        assert!(dcb_all.items.is_empty());

        let item_query = Query::from_items(vec![
            QueryItem::new()
                .with_type("OrderCreated")
                .with_tag("user:42"),
            QueryItem::new().with_type("UserRegistered"),
        ]);

        let dcb_query = convert_query(&item_query);
        assert_eq!(dcb_query.items.len(), 2);
        assert_eq!(dcb_query.items[0].types, vec!["OrderCreated"]);
        assert_eq!(dcb_query.items[0].tags, vec!["user:42"]);
        assert_eq!(dcb_query.items[1].types, vec!["UserRegistered"]);
    }

    #[test]
    fn test_convert_append_condition() {
        let cond = AppendCondition::new(Query::item(QueryItem::new().with_type("OrderShipped")))
            .after(SequencePosition::new(42));

        let dcb_cond = convert_append_condition(&cond);
        assert_eq!(dcb_cond.after, Some(42));
        assert_eq!(dcb_cond.fail_if_events_match.items.len(), 1);
        assert_eq!(
            dcb_cond.fail_if_events_match.items[0].types,
            vec!["OrderShipped"]
        );
    }
}
