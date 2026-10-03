use std::pin::Pin;

use crate::error::Error;
use crate::event::{Event, SequencePosition, SequencedEvent};
use crate::query::{AppendCondition, Query};
use async_trait::async_trait;
use futures_core::Stream;

/// A boxed stream of sequenced events returned by [`EventStore::read`].
pub type EventStream<E> = Pin<Box<dyn Stream<Item = Result<SequencedEvent, Error<E>>> + Send>>;

/// A DCB compliant Event Store.
#[mockall::automock(type Error = std::io::Error;)]
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Error type of the underlying store.
    type Error: Send + Sync + 'static;

    /// Read sequenced events matching the query, starting from the given
    /// sequence position (inclusive), as a stream. `None` starts from the
    /// beginning.
    async fn read(
        &self,
        query: Query,
        position: Option<SequencePosition>,
    ) -> Result<EventStream<Self::Error>, Error<Self::Error>>;

    /// Atomically persist one or more events.
    ///
    /// If an [AppendCondition] is given and the store contains at least one
    /// event matching its query, the append fails with
    /// [`Error::Condition`].
    async fn append(
        &self,
        events: Vec<Event>,
        condition: Option<AppendCondition>,
    ) -> Result<Vec<SequencedEvent>, Error<Self::Error>>;
}
