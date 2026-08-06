use async_trait::async_trait;
use event_sourcing::{Query as EventQuery, SequencedEvent};
use thiserror::Error;

/// Error type for View projection operations.
#[derive(Debug, Error)]
pub enum ViewError {
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("Storage engine error: {0}")]
    Storage(String),

    #[error("Projection execution failed: {0}")]
    Execution(String),
}

/// Unified trait representing a read model (view table) and its event projection logic.
///
/// ### Storage Context (`C`)
/// `C` represents the storage context or database handle used to mutate view tables.
/// It is completely unopinionated and can be any database handle or transaction type, such as:
/// - `&PgPool` or `&mut Transaction<'_, Postgres>` (SQLx / PostgreSQL)
/// - `&DatabaseConnection` or `&DatabaseTransaction` (SeaORM)
/// - `&RedisClient` or `&KvStore`
/// - `()` (no storage context needed)
///
/// Passing a database transaction `&mut Transaction` as `C` enables 100% atomic projections where
/// view table mutations and checkpoint position updates commit together in a single transaction.
#[async_trait]
pub trait View<C = ()>: Send + Sync + 'static {
    /// Name of the view table or read model collection.
    fn view_name(&self) -> &'static str;

    /// Event query filter for subscribed events (defaults to all events).
    fn subscription_query(&self) -> EventQuery {
        EventQuery::all()
    }

    /// Projects an incoming domain event using storage context `ctx` (`&C`).
    ///
    /// The `ctx` parameter provides direct access to your database pool, connection, or transaction
    /// to perform SQL queries or storage updates.
    async fn apply_event(&self, event: &SequencedEvent, ctx: &C) -> Result<(), ViewError>;
}
