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
/// `C` is the arbitrary storage context or database connection (e.g. `&PgPool`, `&Transaction`, `&KvStore`).
#[async_trait]
pub trait View<C = ()>: Send + Sync + 'static {
    /// Name of the view table or read model collection.
    fn view_name(&self) -> &'static str;

    /// Event query filter for subscribed events (defaults to all events).
    fn subscription_query(&self) -> EventQuery {
        EventQuery::all()
    }

    /// Projects an incoming domain event using storage context `C`.
    async fn apply_event(&self, event: &SequencedEvent, ctx: &C) -> Result<(), ViewError>;
}
