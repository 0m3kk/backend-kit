use std::fmt::Display;

use event_store::{EventStore, SequencePosition};
use futures_util::StreamExt;
use thiserror::Error;

use crate::checkpoint::CheckpointStore;
use crate::view::View;

/// Result of one catch-up run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatchupReport {
    /// Number of events applied during this run.
    pub processed: u64,
    /// Highest event position reached.
    pub last_position: SequencePosition,
}

/// Errors produced by the projection worker.
#[derive(Debug, Error)]
pub enum WorkerError {
    /// The event store failed.
    #[error("event store error: {0}")]
    EventStore(String),

    /// The checkpoint store failed.
    #[error("checkpoint error: {0}")]
    Checkpoint(String),

    /// The view failed to apply an event.
    #[error("view error: {0}")]
    View(String),
}

/// Catch-up worker for a single [`View`].
#[derive(Clone)]
pub struct ProjectionWorker<E, C> {
    event_store: E,
    checkpoints: C,
}

impl<E, C> ProjectionWorker<E, C>
where
    E: EventStore + Send + Sync,
    E::Error: Display,
    C: CheckpointStore + Send + Sync,
    C::Error: Display,
{
    /// Creates a worker from an event store and a checkpoint store.
    pub fn new(event_store: E, checkpoints: C) -> Self {
        Self {
            event_store,
            checkpoints,
        }
    }

    /// Replays all unprocessed events for `view`.
    ///
    /// The checkpoint advances every 100 applied events and at the end of a
    /// run. Views that guard writes by event position make crash recovery
    /// idempotent.
    pub async fn catchup<V>(&self, view: &V) -> Result<CatchupReport, WorkerError>
    where
        V: View + Send + Sync,
        V::Error: Display,
    {
        let last_position = self
            .checkpoints
            .load(view.name())
            .await
            .map_err(|error| WorkerError::Checkpoint(error.to_string()))?;

        let from = last_position.map(|position| position.saturating_add(1));
        let mut stream = self
            .event_store
            .read(view.query(), from)
            .await
            .map_err(|error| map_event_store_error(&error))?;

        let mut processed = 0u64;
        let mut highest = last_position.unwrap_or(0);

        while let Some(item) = stream.next().await {
            let sequenced = item.map_err(|error| map_event_store_error(&error))?;
            view.apply(&sequenced)
                .await
                .map_err(|error| WorkerError::View(error.to_string()))?;
            highest = highest.max(sequenced.position);
            processed += 1;

            if processed.is_multiple_of(100) {
                self.save_checkpoint(view.name(), highest).await?;
            }
        }

        if processed > 0 {
            self.save_checkpoint(view.name(), highest).await?;
        }

        Ok(CatchupReport {
            processed,
            last_position: highest,
        })
    }

    async fn save_checkpoint(
        &self,
        view: &str,
        position: SequencePosition,
    ) -> Result<(), WorkerError> {
        self.checkpoints
            .save(view, position)
            .await
            .map_err(|error| WorkerError::Checkpoint(error.to_string()))
    }
}

fn map_event_store_error<E: Display>(error: &event_store::Error<E>) -> WorkerError {
    match error {
        event_store::Error::Condition(condition) => WorkerError::EventStore(format!(
            "append condition failed with {} conflicting event(s)",
            condition.conflicting_events.len()
        )),
        event_store::Error::Store(error) => WorkerError::EventStore(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use event_store::{Event, MockEventStore, Query, SequencedEvent};
    use futures_util::stream;

    use super::*;

    #[derive(Clone, Default)]
    struct FakeView {
        position: Arc<AtomicU64>,
    }

    impl View for FakeView {
        type Error = std::io::Error;

        fn name(&self) -> &str {
            "test.view"
        }

        fn query(&self) -> Query {
            Query::all()
        }

        async fn apply(&self, sequenced: &SequencedEvent) -> Result<(), Self::Error> {
            self.position.store(sequenced.position, Ordering::SeqCst);
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct FakeCheckpointStore {
        position: Arc<AtomicU64>,
    }

    impl FakeCheckpointStore {
        fn position(&self) -> Option<SequencePosition> {
            let position = self.position.load(Ordering::SeqCst);
            (position > 0).then_some(position)
        }
    }

    impl CheckpointStore for FakeCheckpointStore {
        type Error = std::io::Error;

        async fn load(&self, _view: &str) -> Result<Option<SequencePosition>, Self::Error> {
            Ok(self.position())
        }

        async fn save(&self, _view: &str, position: SequencePosition) -> Result<(), Self::Error> {
            self.position.store(position, Ordering::SeqCst);
            Ok(())
        }

        async fn reset(&self, _view: &str) -> Result<(), Self::Error> {
            self.position.store(0, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn dispatches_events_and_resumes_from_checkpoint()
    -> Result<(), Box<dyn std::error::Error>> {
        let sequenced = SequencedEvent {
            event: Event::new("test.event", serde_json::Value::Null, vec![]),
            position: 7,
        };

        let mut event_store = MockEventStore::new();
        event_store
            .expect_read()
            .times(1)
            .returning(move |_, position| {
                assert_eq!(position, Some(4));
                Ok(Box::pin(stream::iter(vec![Ok(sequenced.clone())])))
            });

        let view = FakeView::default();
        let checkpoints = FakeCheckpointStore::default();
        checkpoints.position.store(3, Ordering::SeqCst);

        let worker = ProjectionWorker::new(event_store, checkpoints.clone());
        let report = worker.catchup(&view).await?;

        assert_eq!(report.processed, 1);
        assert_eq!(report.last_position, 7);
        assert_eq!(view.position.load(Ordering::SeqCst), 7);
        assert_eq!(checkpoints.position(), Some(7));
        Ok(())
    }
}
