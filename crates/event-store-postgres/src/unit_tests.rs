#[cfg(test)]
mod tests {
    use crate::*;
    use sqlx::postgres::PgPoolOptions;

    #[test]
    fn test_is_retryable_msg() {
        assert!(is_retryable_msg("error 40001: serialization failure"));
        assert!(is_retryable_msg("error 40P01: deadlock detected"));
        assert!(is_retryable_msg("transient serialization failure occurred"));
        assert!(is_retryable_msg("deadlock detected between transactions"));

        assert!(!is_retryable_msg("unique constraint violation"));
        assert!(!is_retryable_msg("syntax error at or near WHERE"));
        assert!(!is_retryable_msg("connection refused"));
    }

    #[tokio::test]
    async fn test_postgres_store_builder_defaults_and_clamping() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/test_db")
            .unwrap();

        let store = PostgresEventStore::new(pool)
            .with_chunk_size(500)
            .with_max_append_attempts(10);

        assert_eq!(store.chunk_size, 500);
        assert_eq!(store.max_append_attempts, 10);

        // Clamping check for 0 values
        let clamped_store = store.with_chunk_size(0).with_max_append_attempts(0);

        assert_eq!(clamped_store.chunk_size, 1);
        assert_eq!(clamped_store.max_append_attempts, 1);
    }

    #[test]
    fn test_event_row_conversion_to_sequenced_event() {
        let row = EventRow {
            id: "ev-unit-1".to_string(),
            position: 42,
            event_type: "UserRegistered".to_string(),
            data: serde_json::json!({"name": "Alice"}),
            tags: vec!["user:alice".to_string(), "tier:gold".to_string()],
            metadata: Some(serde_json::json!({"ip": "127.0.0.1"})),
            timestamp: 1600000000000,
        };

        let seq_event = SequencedEvent::from(row);
        assert_eq!(seq_event.position.value(), 42);
        assert_eq!(seq_event.timestamp, 1600000000000);
        assert_eq!(seq_event.event.id.as_str(), "ev-unit-1");
        assert_eq!(seq_event.event.event_type.as_str(), "UserRegistered");
        assert_eq!(seq_event.event.tags.len(), 2);
        assert_eq!(seq_event.event.tags[0].as_str(), "user:alice");
        assert_eq!(seq_event.event.tags[1].as_str(), "tier:gold");
        assert!(seq_event.event.metadata.is_some());
    }

    #[test]
    fn test_query_builder_all_query() {
        let mut qb = QueryBuilder::<sqlx::Postgres>::new("SELECT * FROM events");
        let has_where = push_query_filter(&mut qb, &Query::all());
        assert!(!has_where);
        assert_eq!(qb.sql().as_str(), "SELECT * FROM events");
    }

    #[test]
    fn test_query_builder_type_only_filter() {
        let mut qb = QueryBuilder::<sqlx::Postgres>::new("SELECT * FROM events");
        let query = Query::item(QueryItem::new().with_type("OrderCreated"));
        let has_where = push_query_filter(&mut qb, &query);

        assert!(has_where);
        assert!(qb.sql().as_str().contains("WHERE (event_type = ANY("));
    }

    #[test]
    fn test_query_builder_tag_only_filter() {
        let mut qb = QueryBuilder::<sqlx::Postgres>::new("SELECT * FROM events");
        let query = Query::item(QueryItem::new().with_tag("user:42"));
        let has_where = push_query_filter(&mut qb, &query);

        assert!(has_where);
        assert!(qb.sql().as_str().contains("WHERE (tags @>"));
    }

    #[test]
    fn test_query_builder_combined_type_and_tag_filter() {
        let mut qb = QueryBuilder::<sqlx::Postgres>::new("SELECT * FROM events");
        let query = Query::item(
            QueryItem::new()
                .with_type("OrderCreated")
                .with_tag("user:42"),
        );
        let has_where = push_query_filter(&mut qb, &query);

        assert!(has_where);
        assert!(qb.sql().as_str().contains("WHERE ((event_type = ANY("));
        assert!(qb.sql().as_str().contains(") AND tags @>"));
    }

    #[test]
    fn test_query_builder_or_logic_multiple_items() {
        let mut qb = QueryBuilder::<sqlx::Postgres>::new("SELECT * FROM events");
        let query = Query::from_items(vec![
            QueryItem::new().with_type("UserRegistered"),
            QueryItem::new().with_tag("order:100"),
        ]);
        let has_where = push_query_filter(&mut qb, &query);

        assert!(has_where);
        assert!(qb.sql().as_str().contains("WHERE (event_type = ANY("));
        assert!(qb.sql().as_str().contains(" OR tags @>"));
    }

    #[test]
    fn test_query_builder_position_bounds() {
        let mut qb1 = QueryBuilder::<sqlx::Postgres>::new("SELECT * FROM events");
        push_position_bound(&mut qb1, false, ">=", SequencePosition::new(10));
        assert_eq!(
            qb1.sql().as_str(),
            "SELECT * FROM events WHERE position >= $1"
        );

        let mut qb2 = QueryBuilder::<sqlx::Postgres>::new("SELECT * FROM events WHERE 1=1");
        push_position_bound(&mut qb2, true, ">", SequencePosition::new(20));
        assert_eq!(
            qb2.sql().as_str(),
            "SELECT * FROM events WHERE 1=1 AND position > $1"
        );
    }
}
