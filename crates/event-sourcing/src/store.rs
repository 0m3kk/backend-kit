use async_trait::async_trait;
use futures_util::stream::BoxStream;
use thiserror::Error;

use crate::types::{AppendCondition, Event, Query, ReadOptions, SequencedEvent};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReadError {
    #[error("Store error: {0}")]
    StoreError(String),
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum AppendError {
    #[error("Cannot append empty event batch")]
    EmptyBatch,

    #[error(
        "Consistency boundary violation: conflicting event found at sequence position {conflicting_event:?}"
    )]
    Conflict {
        condition: AppendCondition,
        conflicting_event: SequencedEvent,
    },

    #[error("Store error: {0}")]
    StoreError(String),
}

/// Type alias for an async, owned 'static stream of SequencedEvents.
pub type EventStream = BoxStream<'static, Result<SequencedEvent, ReadError>>;

/// Abstract Event Store interface providing DCB compliant read and append operations.
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Reads sequenced events matching a Query, returning an owned 'static stream of SequencedEvent.
    async fn read(&self, query: &Query, options: ReadOptions) -> EventStream;

    /// Atomically appends one or more Events to the store, enforcing the specified AppendCondition if provided.
    async fn append(
        &self,
        events: Vec<Event>,
        condition: Option<AppendCondition>,
    ) -> Result<Vec<SequencedEvent>, AppendError>;
}
