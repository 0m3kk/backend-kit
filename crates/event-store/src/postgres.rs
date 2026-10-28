use std::time::{Duration, Instant};

use async_trait::async_trait;
use opentelemetry::global;
use opentelemetry::metrics::{Counter, Histogram};
use sqlx::pool::PoolConnection;
use sqlx::postgres::{PgConnection, PgPool, Postgres};
use sqlx::{QueryBuilder, Row};

use crate::error::{AppendConditionError, Error};
use crate::event::{Event, SequencePosition, SequencedEvent};
use crate::query::{AppendCondition, Query, QueryItem};
use crate::store::{EventStore, EventStream};

const MAX_RETRIES: u32 = 10;
const BASE_RETRY_DELAY: Duration = Duration::from_millis(20);
const READ_BATCH_SIZE: i64 = 500;

/// Advisory lock key that serializes appends for one event store.
///
/// PostgreSQL sequences allocate a position when the row is inserted, not when
/// the transaction commits. Two concurrent appends can therefore commit in the
/// opposite order of their positions. A projection that advances its checkpoint
/// by position would then skip the event that committed late, permanently.
///
/// Taking this transaction-scoped lock before allocating the position makes
/// commit order match position order. The tradeoff is that appends are
/// serialized per event store, which is the usual price for a totally ordered
/// global log. Use a different database (or a different key in a fork) for each
/// independent event store.
pub const APPEND_LOCK_ID: i64 = 6_432_101_973_551_001;

/// Embedded database migrations.
///
/// Run them once against the database before using the store, e.g.
/// `EVENT_STORE_MIGRATOR.run(&pool).await`. This uses the default
/// `_sqlx_migrations` table, so applications that own migrations of their own
/// should prefer [`migrator`].
pub static EVENT_STORE_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Returns the event store migrations with a dedicated migration table.
///
/// The store keeps its bookkeeping in `_event_store_migrations`, so running
/// these migrations cannot conflict with an application's own migrations in
/// `_sqlx_migrations`.
pub fn migrator() -> sqlx::migrate::Migrator {
    let mut migrator = sqlx::migrate!("./migrations");
    migrator.dangerous_set_table_name("_event_store_migrations");
    migrator
}

#[derive(Clone)]
struct EventStoreMetrics {
    appends_total: Counter<u64>,
    events_appended: Counter<u64>,
    append_duration: Histogram<f64>,
    reads_total: Counter<u64>,
    read_duration: Histogram<f64>,
}

impl Default for EventStoreMetrics {
    fn default() -> Self {
        let meter = global::meter("event-store");
        Self {
            appends_total: meter.u64_counter("db_event_store_appends_total").build(),
            events_appended: meter
                .u64_counter("db_event_store_events_appended_total")
                .build(),
            append_duration: meter
                .f64_histogram("db_event_store_append_duration_seconds")
                .with_unit("s")
                .build(),
            reads_total: meter.u64_counter("db_event_store_reads_total").build(),
            read_duration: meter
                .f64_histogram("db_event_store_read_duration_seconds")
                .with_unit("s")
                .build(),
        }
    }
}

/// A PostgreSQL-backed [Event Store] that is DCB compliant.
#[derive(Debug, Clone)]
pub struct PostgresEventStore {
    pool: PgPool,
}

impl PostgresEventStore {
    /// Create a new PostgreSQL event store with the given connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Runs the embedded event-store migrations against `pool`.
    pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
        migrator().run(pool).await
    }

    /// Runs the embedded migrations and creates a store backed by `pool`.
    pub async fn init(pool: &PgPool) -> Result<Self, sqlx::migrate::MigrateError> {
        Self::migrate(pool).await?;
        Ok(Self::new(pool.clone()))
    }
}

#[async_trait]
impl EventStore for PostgresEventStore {
    type Error = sqlx::Error;

    async fn read(
        &self,
        query: Query,
        position: Option<SequencePosition>,
    ) -> Result<EventStream<Self::Error>, Error<Self::Error>> {
        let pool = self.pool.clone();
        Ok(Box::pin(async_stream::stream! {
            let mut next_from = position
                .map(|p| i64::try_from(p).unwrap_or(i64::MAX))
                .unwrap_or(0);

            loop {
                let mut qb = build_select_query(next_from, &query);
                qb.push(" LIMIT ");
                qb.push_bind(READ_BATCH_SIZE);

                let start = Instant::now();
                let rows_result = {
                    let mut executor = match pool.acquire().await {
                        Ok(e) => e,
                        Err(e) => {
                            yield Err(Error::Store(e));
                            return;
                        }
                    };
                    qb.build().fetch_all(&mut *executor).await
                };

                let rows = match rows_result {
                    Ok(rows) => {
                        let metrics = EventStoreMetrics::default();
                        metrics.reads_total.add(1, &[]);
                        metrics.read_duration.record(start.elapsed().as_secs_f64(), &[]);
                        rows
                    }
                    Err(e) => {
                        yield Err(Error::Store(e));
                        return;
                    }
                };

                let count = rows.len();
                if count == 0 {
                    break;
                }

                for row in rows {
                    let sequenced = row_to_sequenced(row);
                    next_from = (sequenced.position as i64).saturating_add(1);
                    yield Ok(sequenced);
                }

                if (count as i64) < READ_BATCH_SIZE {
                    break;
                }
            }
        }))
    }

    async fn append(
        &self,
        events: Vec<Event>,
        condition: Option<AppendCondition>,
    ) -> Result<Vec<SequencedEvent>, Error<Self::Error>> {
        if events.is_empty() {
            return Ok(Vec::new());
        }

        let start = Instant::now();
        let mut current_events = events;
        let mut current_condition = condition;
        let mut attempts = 0;

        loop {
            attempts += 1;
            let is_last_attempt = attempts >= MAX_RETRIES;

            let try_events = if is_last_attempt {
                std::mem::take(&mut current_events)
            } else {
                current_events.clone()
            };
            let try_condition = if is_last_attempt {
                current_condition.take()
            } else {
                current_condition.clone()
            };

            let conn = self.pool.acquire().await.map_err(Error::Store)?;
            let mut tx = AppendTransaction::begin(conn, try_condition.is_some())
                .await
                .map_err(Error::Store)?;

            // Acquire before the insert so the sequence value and the commit
            // are ordered with every other append to this store.
            if let Err(error) = acquire_append_lock(tx.conn()).await {
                tx.rollback().await;
                return Err(Error::Store(error));
            }

            let result = match append_events_within_transaction(
                tx.conn(),
                try_events,
                try_condition,
            )
            .await
            {
                Ok(sequenced) => tx.commit().await.map(|()| sequenced).map_err(Error::Store),
                Err(e) => {
                    tx.rollback().await;
                    Err(e)
                }
            };

            match result {
                Ok(sequenced) => {
                    let metrics = EventStoreMetrics::default();
                    metrics.appends_total.add(1, &[]);
                    metrics.events_appended.add(sequenced.len() as u64, &[]);
                    metrics
                        .append_duration
                        .record(start.elapsed().as_secs_f64(), &[]);
                    return Ok(sequenced);
                }
                Err(e) => {
                    if matches!(e, Error::Condition(_)) {
                        return Err(e);
                    }

                    if !is_last_attempt && is_retryable_conflict(&e) {
                        let delay = BASE_RETRY_DELAY.saturating_mul(1u32 << (attempts - 1).min(5));
                        tokio::time::sleep(delay).await;
                        continue;
                    }

                    return Err(e);
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Write Path Helpers
// -----------------------------------------------------------------------------

/// Transaction used by the append path.
///
/// `sqlx::Transaction` sends a `ROLLBACK` when a `COMMIT` fails. PostgreSQL
/// already ended the transaction when `COMMIT` returns an error (a
/// serialization failure, for example), so that extra `ROLLBACK` logs
/// `there is no transaction in progress` on every retried append.
///
/// This guard commits explicitly, rolls back only while the transaction is
/// still open, and closes the session instead of returning a poisoned
/// connection to the pool when its future is cancelled.
struct AppendTransaction {
    conn: Option<PoolConnection<Postgres>>,
    open: bool,
}

impl AppendTransaction {
    async fn begin(
        conn: PoolConnection<Postgres>,
        serializable: bool,
    ) -> Result<Self, sqlx::Error> {
        let mut tx = Self {
            conn: Some(conn),
            open: false,
        };

        let statement = if serializable {
            "BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE"
        } else {
            "BEGIN"
        };

        sqlx::query(statement).execute(tx.conn()).await?;
        tx.open = true;
        Ok(tx)
    }

    fn conn(&mut self) -> &mut PgConnection {
        match self.conn.as_mut() {
            Some(conn) => conn,
            None => unreachable!("append transaction connection was already released"),
        }
    }

    async fn commit(mut self) -> Result<(), sqlx::Error> {
        let result = sqlx::query("COMMIT").execute(self.conn()).await;

        // A failed `COMMIT` still ends the transaction on the server. Closing
        // the guard here prevents a redundant `ROLLBACK` from being sent.
        self.open = false;
        result.map(|_| ())
    }

    async fn rollback(mut self) {
        let result = sqlx::query("ROLLBACK").execute(self.conn()).await;

        // Keep `open` when the rollback itself failed so `Drop` closes the
        // session instead of returning a connection with an open transaction.
        if result.is_ok() {
            self.open = false;
        }
    }
}

impl Drop for AppendTransaction {
    fn drop(&mut self) {
        if !self.open {
            return;
        }

        if let Some(conn) = self.conn.take() {
            // Closing the session rolls back and keeps the pool clean when the
            // append future is cancelled between `BEGIN` and `COMMIT`.
            drop(conn.detach());
        }
    }
}

async fn append_events_within_transaction(
    conn: &mut PgConnection,
    events: Vec<Event>,
    condition: Option<AppendCondition>,
) -> Result<Vec<SequencedEvent>, Error<sqlx::Error>> {
    if let Some(condition) = condition {
        enforce_append_condition(conn, condition).await?;
    }

    insert_events(conn, events).await
}

async fn enforce_append_condition(
    conn: &mut PgConnection,
    condition: AppendCondition,
) -> Result<(), Error<sqlx::Error>> {
    let from = match condition.after {
        Some(after) => i64::try_from(after)
            .map(|a| a.saturating_add(1))
            .unwrap_or(i64::MAX),
        None => 0,
    };

    let mut qb = build_select_query(from, &condition.fail_if_events_match);
    qb.push(" LIMIT 1");

    let conflicting = qb
        .build()
        .fetch_optional(conn)
        .await
        .map_err(Error::Store)?
        .map(row_to_sequenced);

    if let Some(event) = conflicting {
        return Err(Error::Condition(AppendConditionError {
            conflicting_events: vec![event],
        }));
    }

    Ok(())
}

async fn acquire_append_lock(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(APPEND_LOCK_ID)
        .execute(conn)
        .await?;
    Ok(())
}

/// Inserts events one at a time so each returned position is unambiguously
/// associated with its event.
///
/// A single multi-row `INSERT ... RETURNING position` does not guarantee that
/// rows are returned in `VALUES` order, so zipping the input with the result
/// could pair an event with another event's position. The append lock already
/// serializes writers, so the extra round trips are acceptable for correctness.
async fn insert_events(
    conn: &mut PgConnection,
    events: Vec<Event>,
) -> Result<Vec<SequencedEvent>, Error<sqlx::Error>> {
    let mut sequenced = Vec::with_capacity(events.len());

    for event in events {
        let row = sqlx::query(
            "INSERT INTO events (event_type, data, metadata, tags) \
             VALUES ($1, $2, $3, $4) RETURNING position",
        )
        .bind(&event.event_type)
        .bind(&event.data)
        .bind(&event.metadata)
        .bind(&event.tags)
        .fetch_one(&mut *conn)
        .await
        .map_err(Error::Store)?;

        let position = row.get::<i64, _>(0) as u64;
        sequenced.push(SequencedEvent { event, position });
    }

    Ok(sequenced)
}

// -----------------------------------------------------------------------------
// Read Path Helpers & Utilities
// -----------------------------------------------------------------------------

fn build_select_query(from: i64, query: &Query) -> QueryBuilder<Postgres> {
    let mut qb = QueryBuilder::new(
        "SELECT position, event_type, data, metadata, tags FROM events WHERE position >= ",
    );
    qb.push_bind(from);

    match query {
        Query::All => {}
        Query::Items(items) if items.is_empty() => {
            qb.push(" AND FALSE");
        }
        Query::Items(items) => {
            qb.push(" AND (");
            let mut first = true;
            for item in items {
                if !first {
                    qb.push(" OR ");
                }
                qb.push("(");
                push_item_sql(&mut qb, item);
                qb.push(")");
                first = false;
            }
            qb.push(")");
        }
    }

    qb.push(" ORDER BY position");
    qb
}

fn push_item_sql(qb: &mut QueryBuilder<Postgres>, item: &QueryItem) {
    let mut first = true;

    if !item.types.is_empty() {
        qb.push("event_type = ANY(");
        qb.push_bind(item.types.as_slice());
        qb.push(")");
        first = false;
    }

    if !item.tags.is_empty() {
        if !first {
            qb.push(" AND ");
        }
        qb.push("tags @> ");
        qb.push_bind(item.tags.as_slice());
        first = false;
    }

    if first {
        qb.push("TRUE");
    }
}

fn row_to_sequenced(row: sqlx::postgres::PgRow) -> SequencedEvent {
    SequencedEvent {
        position: row.get::<i64, _>(0) as u64,
        event: Event {
            event_type: row.get(1),
            data: row.get(2),
            metadata: row.get(3),
            tags: row.get(4),
        },
    }
}

fn is_retryable_conflict(error: &Error<sqlx::Error>) -> bool {
    if let Error::Store(sqlx::Error::Database(db_error)) = error {
        return matches!(db_error.code().as_deref(), Some("40001" | "40P01"));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(types: &[&str], tags: &[&str]) -> QueryItem {
        QueryItem::new(
            types.iter().map(|t| t.to_string()).collect(),
            tags.iter().map(|t| t.to_string()).collect(),
        )
    }

    fn sql_of(qb: QueryBuilder<Postgres>) -> String {
        qb.into_string()
    }

    #[test]
    fn select_all_has_no_filter() {
        let sql = sql_of(build_select_query(10, &Query::all()));

        assert!(sql.contains("WHERE position >= "));
        assert!(sql.contains("ORDER BY position"));
        assert!(!sql.contains("AND ("));
    }

    #[test]
    fn select_types_only_item() {
        let query = Query::from_items([item(&["created", "updated"], &[])]);
        let sql = sql_of(build_select_query(0, &query));

        assert!(sql.contains("AND ("));
        assert!(sql.contains("event_type = ANY("));
    }

    #[test]
    fn select_tags_only_item() {
        let query = Query::from_items([item(&[], &["tag1", "tag2"])]);
        let sql = sql_of(build_select_query(0, &query));

        assert!(sql.contains("tags @> "));
        assert!(!sql.contains("event_type = ANY"));
    }

    #[test]
    fn select_type_and_tags_item() {
        let query = Query::from_items([item(&["created"], &["tag1"])]);
        let sql = sql_of(build_select_query(0, &query));

        assert!(sql.contains("event_type = ANY("));
        assert!(sql.contains("AND tags @> "));
    }

    #[test]
    fn select_items_combined_with_or() {
        let query = Query::from_items([item(&["created"], &[]), item(&["deleted"], &[])]);
        let sql = sql_of(build_select_query(0, &query));

        assert!(sql.contains(" OR "));
        assert_eq!(sql.matches("event_type = ANY").count(), 2);
    }

    #[test]
    fn select_empty_items_matches_nothing() {
        let query = Query::from_items([]);
        let sql = sql_of(build_select_query(5, &query));

        assert!(sql.contains("AND FALSE"));
    }

    #[test]
    fn select_empty_item_matches_everything() {
        let query = Query::from_items([item(&[], &[])]);
        let sql = sql_of(build_select_query(0, &query));

        assert!(sql.contains("TRUE"));
    }
}
