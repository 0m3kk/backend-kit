//! Object-safe projection contract for the multi-view worker.

use std::future::Future;
use std::pin::Pin;

use event_store::{Query, SequencedEvent};
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
    #[error("projection reset failed: {0}")]
    Reset(String),
}

/// A read model that the supervisor can run without generic type parameters.
///
/// Implementations translate events into rows. The default [`apply_batch`]
/// applies events one by one; implementations may override it to use a single
/// transaction and checkpoint per batch.
///
/// [`apply_batch`]: Projection::apply_batch
pub trait Projection: Send + Sync {
    /// Stable projection name used as the checkpoint key.
    fn name(&self) -> &'static str;

    /// Event-store query for the events this projection consumes.
    fn query(&self) -> Query;

    /// Applies one event to the projection.
    fn apply<'a>(
        &'a self,
        event: &'a SequencedEvent,
    ) -> ProjectionFuture<'a, Result<(), ProjectionError>>;

    /// Applies a batch of events.
    ///
    /// The default implementation applies events sequentially. A projection
    /// that owns its SQL transaction can override this to make the batch
    /// atomic.
    fn apply_batch<'a>(
        &'a self,
        events: &'a [SequencedEvent],
    ) -> ProjectionFuture<'a, Result<(), ProjectionError>> {
        Box::pin(async move {
            for event in events {
                self.apply(event).await?;
            }
            Ok(())
        })
    }

    /// Clears the projection's rows before a rebuild.
    fn reset(&self) -> ProjectionFuture<'_, Result<(), ProjectionError>>;
}
