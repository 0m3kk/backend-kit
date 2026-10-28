//! Object-safe projection contract for the multi-view worker.

use std::future::Future;
use std::pin::Pin;

use event_store::{Query, SequencePosition, SequencedEvent};
use thiserror::Error;

/// Boxed future returned by [`Projection`] methods.
pub type ProjectionFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Errors produced while applying or resetting a projection.
#[derive(Debug, Error)]
pub enum ProjectionError {
    /// An event payload could not be deserialized.
    #[error("serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    /// An event type outside the projection's query was applied.
    #[error("unexpected event type `{0}`")]
    UnexpectedEvent(String),

    /// The projection store rejected the write.
    #[error("projection store failed: {0}")]
    Store(String),

    /// Resetting the projection failed.
    #[error("reset failed: {0}")]
    Reset(String),
}

/// A read model that the supervisor can run without generic type parameters.
///
/// The projection owns its checkpoint write. [`apply_batch`] receives the
/// highest event position of the batch and must persist it together with the
/// read-model writes, so a batch is exactly-once: a crash commits either both
/// the rows and the checkpoint or neither.
///
/// Implementations backed by PostgreSQL should use
/// [`save_in_transaction`](crate::save_in_transaction) inside the same
/// transaction as the row writes.
///
/// [`apply_batch`]: Projection::apply_batch
pub trait Projection: Send + Sync {
    /// Stable projection name used as the checkpoint key.
    fn name(&self) -> &'static str;

    /// Event-store query for the events this projection consumes.
    fn query(&self) -> Query;

    /// Applies a batch of events and persists `checkpoint`.
    ///
    /// `checkpoint` is the highest position in `events` (or the last applied
    /// position when `events` is empty). Implementations must not advance a
    /// checkpoint past a write that was rolled back.
    fn apply_batch<'a>(
        &'a self,
        events: &'a [SequencedEvent],
        checkpoint: SequencePosition,
    ) -> ProjectionFuture<'a, Result<(), ProjectionError>>;

    /// Clears the projection's rows before a rebuild.
    fn reset(&self) -> ProjectionFuture<'_, Result<(), ProjectionError>>;
}
