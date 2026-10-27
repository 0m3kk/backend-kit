//! Notification test for the PostgreSQL event store.

#![allow(clippy::unwrap_used)]

use std::time::Duration;

use event_store::{EVENTS_NOTIFY_CHANNEL, Event, EventStore, PostgresEventStore};
use sqlx::PgPool;
use sqlx::postgres::PgListener;

#[sqlx::test]
async fn append_emits_notification(pool: PgPool) {
    let mut listener = PgListener::connect_with(&pool).await.unwrap();
    listener.listen(EVENTS_NOTIFY_CHANNEL).await.unwrap();

    let store = PostgresEventStore::new(pool);
    store
        .append(
            vec![Event::new(
                "product_created",
                serde_json::Value::Null,
                vec!["product:1".into()],
            )],
            None,
        )
        .await
        .unwrap();

    let notification = tokio::time::timeout(Duration::from_secs(2), listener.recv())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(notification.channel(), EVENTS_NOTIFY_CHANNEL);
    assert_eq!(notification.payload(), "product_created");
}
