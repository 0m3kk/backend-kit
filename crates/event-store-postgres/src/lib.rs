use async_trait::async_trait;
use futures_util::StreamExt;
use sqlx::{QueryBuilder, Transaction, postgres::PgPool};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub use event_sourcing::*;

/// Default number of attempts for an append before giving up on transient serialization failures.
pub const DEFAULT_MAX_APPEND_ATTEMPTS: u32 = 5;

/// Default batch chunk size for multi-row PostgreSQL INSERT statements.
pub const DEFAULT_INSERT_CHUNK_SIZE: usize = 1000;

/// Internal representation of a single row in the `events` table.
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct EventRow {
    id: String,
    position: i64,
    event_type: String,
    data: serde_json::Value,
    tags: Vec<String>,
    metadata: Option<serde_json::Value>,
    timestamp: i64,
}

impl From<EventRow> for SequencedEvent {
    fn from(row: EventRow) -> Self {
        let tags = row.tags.into_iter().map(Tag::new).collect();
        let mut event = Event::new(row.id, row.event_type, row.data, tags);
        if let Some(meta) = row.metadata {
            event = event.with_metadata(meta);
        }

        SequencedEvent::new(
            SequencePosition::new(row.position as u64),
            row.timestamp as u64,
            event,
        )
    }
}

/// A production-grade PostgreSQL implementation of [`EventStore`].
#[derive(Debug, Clone)]
pub struct PostgresEventStore {
    pool: PgPool,
    table_name: String,
    max_append_attempts: u32,
    chunk_size: usize,
}

impl PostgresEventStore {
    /// Creates a store backed by an existing connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            table_name: "events".to_string(),
            max_append_attempts: DEFAULT_MAX_APPEND_ATTEMPTS,
            chunk_size: DEFAULT_INSERT_CHUNK_SIZE,
        }
    }

    /// Creates a store with a custom table name.
    pub fn with_table(pool: PgPool, table_name: impl Into<String>) -> Self {
        Self {
            pool,
            table_name: table_name.into(),
            max_append_attempts: DEFAULT_MAX_APPEND_ATTEMPTS,
            chunk_size: DEFAULT_INSERT_CHUNK_SIZE,
        }
    }

    /// Overrides maximum append retry attempts for transient serialization failures (builder style).
    pub fn with_max_append_attempts(mut self, attempts: u32) -> Self {
        self.max_append_attempts = attempts.max(1);
        self
    }

    /// Overrides maximum chunk size for batch inserts.
    pub fn with_chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size.max(1);
        self
    }

    /// Returns a reference to the underlying PgPool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Runs embedded database migrations, creating the `events` table and indices automatically.
    pub async fn migrate(&self) -> Result<(), sqlx::migrate::MigrateError> {
        sqlx::migrate!("./migrations").run(&self.pool).await
    }

    /// Performs a single append attempt inside a SERIALIZABLE transaction.
    async fn try_append_once(
        &self,
        events: &[Event],
        condition: Option<&AppendCondition>,
    ) -> Result<Vec<SequencedEvent>, AppendError> {
        let mut tx = self
            .pool
            .begin_with("BEGIN ISOLATION LEVEL SERIALIZABLE")
            .await
            .map_err(|e| AppendError::StoreError(e.to_string()))?;

        // 1. Enforce AppendCondition if specified
        if let Some(cond) = condition
            && has_conflict(
                &mut tx,
                &self.table_name,
                &cond.fail_if_events_match,
                cond.after,
            )
            .await
            .map_err(|e| AppendError::StoreError(e.to_string()))?
        {
            // Fetch conflicting event details for error reporting
            let conflicting_event = fetch_conflicting_event(
                &mut tx,
                &self.table_name,
                &cond.fail_if_events_match,
                cond.after,
            )
            .await
            .map_err(|e| AppendError::StoreError(e.to_string()))?;

            return Err(AppendError::Conflict {
                condition: cond.clone(),
                conflicting_event,
            });
        }

        // 2. Perform chunked multi-row batch INSERTs
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let total_events = events.len();
        let mut appended = Vec::with_capacity(total_events);

        for chunk in events.chunks(self.chunk_size) {
            let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(format!(
                "INSERT INTO {} (id, event_type, data, tags, metadata, timestamp) ",
                self.table_name
            ));

            qb.push_values(chunk, |mut b, event| {
                let tags: Vec<String> = event.tags.iter().map(|t| t.0.clone()).collect();
                b.push_bind(event.id.as_str())
                    .push_bind(event.event_type.as_str())
                    .push_bind(&event.data)
                    .push_bind(tags)
                    .push_bind(&event.metadata)
                    .push_bind(timestamp);
            });

            qb.push(" RETURNING position");

            let chunk_positions = qb
                .build_query_scalar::<i64>()
                .fetch_all(&mut *tx)
                .await
                .map_err(|e| AppendError::StoreError(e.to_string()))?;

            for (event, pos_raw) in chunk.iter().zip(chunk_positions) {
                let seq_event = SequencedEvent::new(
                    SequencePosition::new(pos_raw as u64),
                    timestamp as u64,
                    event.clone(),
                );
                appended.push(seq_event);
            }
        }

        tx.commit()
            .await
            .map_err(|e| AppendError::StoreError(e.to_string()))?;

        Ok(appended)
    }
}

#[async_trait]
impl EventStore for PostgresEventStore {
    async fn read(&self, query: &Query, options: ReadOptions) -> EventStream {
        let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(format!(
            "SELECT id, position, event_type, data, tags, metadata, timestamp FROM {}",
            self.table_name
        ));

        let has_where = push_query_filter(&mut qb, query);
        if let Some(after) = options.after {
            push_position_bound(&mut qb, has_where, ">", after);
        }

        match options.direction {
            Direction::Forward => qb.push(" ORDER BY position ASC"),
            Direction::Backward => qb.push(" ORDER BY position DESC"),
        };

        if let Some(limit) = options.limit {
            qb.push(" LIMIT ").push_bind(limit as i64);
        }

        let pool = self.pool.clone();

        let stream = async_stream::stream! {
            let mut rows = qb.build_query_as::<EventRow>().fetch(&pool);
            while let Some(res) = rows.next().await {
                yield res
                    .map(SequencedEvent::from)
                    .map_err(|e| ReadError::StoreError(e.to_string()));
            }
        };

        Box::pin(stream)
    }

    async fn append(
        &self,
        events: Vec<Event>,
        condition: Option<AppendCondition>,
    ) -> Result<Vec<SequencedEvent>, AppendError> {
        if events.is_empty() {
            return Err(AppendError::EmptyBatch);
        }

        let mut attempt: u32 = 0;
        loop {
            attempt += 1;

            match self.try_append_once(&events, condition.as_ref()).await {
                Ok(result) => return Ok(result),
                Err(AppendError::StoreError(ref err_msg))
                    if is_retryable_msg(err_msg) && attempt < self.max_append_attempts =>
                {
                    backoff(attempt).await;
                    continue;
                }
                Err(err) => return Err(err),
            }
        }
    }
}

/// Appends the SQL filter for a [`Query`] to `qb` using owned String binds.
fn push_query_filter<'a>(qb: &mut QueryBuilder<'a, sqlx::Postgres>, query: &Query) -> bool {
    let Query::Items { items } = query else {
        return false;
    };
    if items.is_empty() {
        return false;
    }
    qb.push(" WHERE (");
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            qb.push(" OR ");
        }
        push_item(qb, item);
    }
    qb.push(")");
    true
}

/// Appends a `position <op> <pos>` predicate to `qb`.
fn push_position_bound<'a>(
    qb: &mut QueryBuilder<'a, sqlx::Postgres>,
    has_where: bool,
    op: &'static str,
    pos: SequencePosition,
) {
    qb.push(if has_where { " AND " } else { " WHERE " });
    qb.push("position ")
        .push(op)
        .push(" ")
        .push_bind(pos.value() as i64);
}

/// Appends the SQL predicate for a single [`QueryItem`] to `qb` binding owned values.
fn push_item<'a>(qb: &mut QueryBuilder<'a, sqlx::Postgres>, item: &QueryItem) {
    let has_types = !item.types.is_empty();
    let has_tags = !item.tags.is_empty();

    match (has_types, has_tags) {
        (false, false) => {
            qb.push("TRUE");
        }
        (true, false) => {
            let types: Vec<String> = item.types.iter().map(|t| t.0.clone()).collect();
            qb.push("event_type = ANY(").push_bind(types).push(")");
        }
        (false, true) => {
            let tags: Vec<String> = item.tags.iter().map(|t| t.0.clone()).collect();
            qb.push("tags @> ").push_bind(tags);
        }
        (true, true) => {
            let types: Vec<String> = item.types.iter().map(|t| t.0.clone()).collect();
            let tags: Vec<String> = item.tags.iter().map(|t| t.0.clone()).collect();
            qb.push("(event_type = ANY(")
                .push_bind(types)
                .push(") AND tags @> ")
                .push_bind(tags)
                .push(")");
        }
    }
}

/// Returns `true` if the store already contains an event matching `query` with position > `after`.
async fn has_conflict(
    tx: &mut Transaction<'_, sqlx::Postgres>,
    table_name: &str,
    query: &Query,
    after: Option<SequencePosition>,
) -> Result<bool, sqlx::Error> {
    let mut qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new(format!("SELECT EXISTS(SELECT 1 FROM {}", table_name));

    let has_where = push_query_filter(&mut qb, query);
    if let Some(after) = after {
        push_position_bound(&mut qb, has_where, ">", after);
    }

    qb.push(")");

    let exists: bool = qb.build_query_scalar::<bool>().fetch_one(&mut **tx).await?;
    Ok(exists)
}

/// Fetches details of the conflicting event for error reporting.
async fn fetch_conflicting_event(
    tx: &mut Transaction<'_, sqlx::Postgres>,
    table_name: &str,
    query: &Query,
    after: Option<SequencePosition>,
) -> Result<SequencedEvent, sqlx::Error> {
    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(format!(
        "SELECT id, position, event_type, data, tags, metadata, timestamp FROM {}",
        table_name
    ));

    let has_where = push_query_filter(&mut qb, query);
    if let Some(after) = after {
        push_position_bound(&mut qb, has_where, ">", after);
    }

    qb.push(" ORDER BY position ASC LIMIT 1");

    let row = qb.build_query_as::<EventRow>().fetch_one(&mut **tx).await?;
    Ok(SequencedEvent::from(row))
}

/// Returns true for transient serialization failures (40001) and deadlock detected (40P01).
fn is_retryable_msg(err_msg: &str) -> bool {
    err_msg.contains("40001")
        || err_msg.contains("40P01")
        || err_msg.contains("serialization failure")
        || err_msg.contains("deadlock")
}

/// Sleep for exponential backoff between append retries.
async fn backoff(attempt: u32) {
    let base = Duration::from_millis(5);
    let delay = base.saturating_mul(2u32.saturating_pow(attempt.saturating_sub(1)));
    tokio::time::sleep(delay.min(Duration::from_millis(200))).await;
}

#[cfg(test)]
mod unit_tests;
