use std::future::Future;

use event_store::{Query, SequencedEvent};

/// A persisted read model that consumes events from the event store.
pub trait View: Send + Sync {
    /// Error type chosen by the implementation.
    type Error;

    /// Stable view name used as the checkpoint key.
    fn name(&self) -> &str;

    /// Event-store query for the events this view consumes.
    fn query(&self) -> Query;

    /// Applies one sequenced event to the view.
    fn apply(
        &self,
        sequenced: &SequencedEvent,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}
