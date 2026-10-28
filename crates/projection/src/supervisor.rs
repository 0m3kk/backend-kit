//! Multi-projection supervisor: one catch-up loop per view.

use std::fmt::Display;
use std::sync::Arc;

use event_store::EventStore;
use futures_util::StreamExt;
use thiserror::Error;
use tokio::sync::{broadcast, watch};
use tokio::task::JoinSet;
use tokio::time::{self, MissedTickBehavior};

use crate::checkpoint::CheckpointStore;
use crate::health::WorkerHealth;
use crate::projection::{Projection, ProjectionError};
use crate::runner::CatchupConfig;

/// Runtime configuration for [`ProjectionSupervisor`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerConfig {
    catchup: CatchupConfig,
    batch_size: usize,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            catchup: CatchupConfig::default(),
            batch_size: 100,
        }
    }
}

impl WorkerConfig {
    /// Creates a configuration with the given catch-up settings.
    pub fn new(catchup: CatchupConfig) -> Self {
        Self {
            catchup,
            ..Self::default()
        }
    }

    /// Sets the maximum number of events applied per batch.
    #[must_use]
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size.max(1);
        self
    }

    /// Returns the catch-up settings.
    pub fn catchup(&self) -> CatchupConfig {
        self.catchup
    }

    /// Returns the maximum batch size.
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }
}

/// Errors produced by the projection supervisor.
#[derive(Debug, Error)]
pub enum SupervisorError {
    /// Reading events failed.
    #[error("event store error: {0}")]
    EventStore(String),

    /// Loading or saving a checkpoint failed.
    #[error("checkpoint error: {0}")]
    Checkpoint(String),

    /// A projection failed to apply a batch.
    #[error("projection `{projection}` failed: {source}")]
    Projection {
        /// Projection name.
        projection: String,
        /// Underlying projection error.
        #[source]
        source: ProjectionError,
    },

    /// A rebuild was requested for an unknown projection.
    #[error("projection `{0}` not found")]
    UnknownProjection(String),

    /// A supervisor task terminated unexpectedly.
    #[error("worker task failed: {0}")]
    Task(String),
}

fn map_event_store_error<E: Display>(error: &event_store::Error<E>) -> SupervisorError {
    match error {
        event_store::Error::Condition(_) => {
            SupervisorError::EventStore("append condition failed".to_string())
        }
        event_store::Error::Store(error) => SupervisorError::EventStore(error.to_string()),
    }
}

/// Runs one catch-up loop per projection against a shared event store.
pub struct ProjectionSupervisor<E, C> {
    event_store: E,
    checkpoints: C,
    projections: Vec<Arc<dyn Projection>>,
    config: WorkerConfig,
    health: WorkerHealth,
}

impl<E, C> ProjectionSupervisor<E, C> {
    /// Creates a supervisor for the given projections.
    pub fn new(
        event_store: E,
        checkpoints: C,
        projections: Vec<Arc<dyn Projection>>,
        config: WorkerConfig,
    ) -> Self {
        let health = WorkerHealth::new(
            projections
                .iter()
                .map(|projection| projection.name().to_string()),
        );

        Self {
            event_store,
            checkpoints,
            projections,
            config,
            health,
        }
    }

    /// Returns the shared health snapshot.
    pub fn health(&self) -> WorkerHealth {
        self.health.clone()
    }
}

impl<E, C> ProjectionSupervisor<E, C>
where
    C: CheckpointStore + Send + Sync,
    C::Error: Display,
{
    /// Clears one projection and its checkpoint so the next run replays from
    /// the beginning.
    pub async fn rebuild(&self, name: &str) -> Result<(), SupervisorError> {
        let projection = self
            .projections
            .iter()
            .find(|projection| projection.name() == name)
            .ok_or_else(|| SupervisorError::UnknownProjection(name.to_string()))?;

        projection
            .reset()
            .await
            .map_err(|source| SupervisorError::Projection {
                projection: name.to_string(),
                source,
            })?;

        self.checkpoints
            .reset(name)
            .await
            .map_err(|error| SupervisorError::Checkpoint(error.to_string()))?;

        Ok(())
    }
}

impl<E, C> ProjectionSupervisor<E, C>
where
    E: EventStore + Clone + Send + Sync + 'static,
    E::Error: Display,
    C: CheckpointStore + Clone + Send + Sync + 'static,
    C::Error: Display,
{
    /// Runs every projection loop until `shutdown` completes.
    ///
    /// `wake` is a broadcast hint channel. Each projection gets its own
    /// receiver; a missed notification only delays catch-up until the polling
    /// ticker fires.
    pub async fn run(
        self,
        wake: broadcast::Receiver<()>,
        shutdown: watch::Receiver<bool>,
    ) -> Result<(), SupervisorError> {
        let Self {
            event_store,
            checkpoints,
            projections,
            config,
            health,
        } = self;

        if projections.is_empty() {
            let mut shutdown = shutdown;
            let _ = shutdown.changed().await;
            return Ok(());
        }

        let mut tasks = JoinSet::new();

        for projection in projections {
            let event_store = event_store.clone();
            let checkpoints = checkpoints.clone();
            let health = health.clone();
            let projection_wake = wake.resubscribe();
            let projection_shutdown = shutdown.clone();

            tasks.spawn(async move {
                run_projection_loop(
                    event_store,
                    checkpoints,
                    projection,
                    config,
                    projection_wake,
                    projection_shutdown,
                    health,
                )
                .await;
            });
        }

        while let Some(result) = tasks.join_next().await {
            if let Err(error) = result {
                return Err(SupervisorError::Task(error.to_string()));
            }
        }

        Ok(())
    }
}

async fn run_projection_loop<E, C>(
    event_store: E,
    checkpoints: C,
    projection: Arc<dyn Projection>,
    config: WorkerConfig,
    mut wake: broadcast::Receiver<()>,
    mut shutdown: watch::Receiver<bool>,
    health: WorkerHealth,
) where
    E: EventStore + Send + Sync,
    E::Error: Display,
    C: CheckpointStore + Send + Sync,
    C::Error: Display,
{
    let name = projection.name();
    let mut ticker = time::interval(config.catchup.interval());
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    tracing::info!(
        view = name,
        interval = ?config.catchup.interval(),
        batch_size = config.batch_size(),
        "projection supervisor loop started"
    );

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                catchup_and_record(
                    &event_store,
                    &checkpoints,
                    projection.as_ref(),
                    config.batch_size(),
                    &health,
                ).await;
            }
            wake_result = wake.recv() => {
                match wake_result {
                    Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {
                        if !config.catchup.debounce().is_zero() {
                            time::sleep(config.catchup.debounce()).await;
                        }
                        catchup_and_record(
                            &event_store,
                            &checkpoints,
                            projection.as_ref(),
                            config.batch_size(),
                            &health,
                        ).await;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        // Keep the polling ticker alive; notifications are only hints.
                    }
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
        }
    }

    tracing::info!(view = name, "projection supervisor loop stopped");
}

async fn catchup_and_record<E, C>(
    event_store: &E,
    checkpoints: &C,
    projection: &dyn Projection,
    batch_size: usize,
    health: &WorkerHealth,
) where
    E: EventStore + Send + Sync,
    E::Error: Display,
    C: CheckpointStore + Send + Sync,
    C::Error: Display,
{
    if let Err(error) = catchup_once(event_store, checkpoints, projection, batch_size, health).await
    {
        health
            .record_error(projection.name(), &error.to_string())
            .await;
        tracing::error!(
            view = projection.name(),
            %error,
            "projection catch-up failed"
        );
    }
}

async fn catchup_once<E, C>(
    event_store: &E,
    checkpoints: &C,
    projection: &dyn Projection,
    batch_size: usize,
    health: &WorkerHealth,
) -> Result<(), SupervisorError>
where
    E: EventStore + Send + Sync,
    E::Error: Display,
    C: CheckpointStore + Send + Sync,
    C::Error: Display,
{
    let last_position = checkpoints
        .load(projection.name())
        .await
        .map_err(|error| SupervisorError::Checkpoint(error.to_string()))?;
    let from = last_position.map(|position| position.saturating_add(1));

    let mut stream = event_store
        .read(projection.query(), from)
        .await
        .map_err(|error| map_event_store_error(&error))?;

    let mut events = Vec::with_capacity(batch_size);
    let mut highest = last_position.unwrap_or(0);

    while events.len() < batch_size {
        match stream.next().await {
            Some(Ok(event)) => {
                highest = highest.max(event.position);
                events.push(event);
            }
            Some(Err(error)) => return Err(map_event_store_error(&error)),
            None => break,
        }
    }

    if events.is_empty() {
        health.record_success(projection.name(), highest, 0).await;
        return Ok(());
    }

    let processed = events.len() as u64;
    projection
        .apply_batch(&events)
        .await
        .map_err(|source| SupervisorError::Projection {
            projection: projection.name().to_string(),
            source,
        })?;

    checkpoints
        .save(projection.name(), highest)
        .await
        .map_err(|error| SupervisorError::Checkpoint(error.to_string()))?;

    health
        .record_success(projection.name(), highest, processed)
        .await;

    tracing::info!(
        view = projection.name(),
        processed,
        last_position = highest,
        "projection caught up"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use event_store::{
        AppendCondition, Error, Event, EventStore, EventStream, Query, SequencePosition,
        SequencedEvent,
    };
    use futures_util::stream;
    use tokio::sync::watch;

    use super::*;
    use crate::projection::ProjectionFuture;

    #[derive(Clone, Default)]
    struct TestEventStore {
        events: Arc<Vec<SequencedEvent>>,
    }

    #[async_trait::async_trait]
    impl EventStore for TestEventStore {
        type Error = std::io::Error;

        async fn read(
            &self,
            _query: Query,
            position: Option<SequencePosition>,
        ) -> Result<EventStream<Self::Error>, Error<Self::Error>> {
            let from = position.unwrap_or(0);
            let events = self
                .events
                .iter()
                .filter(|event| event.position >= from)
                .cloned()
                .collect::<Vec<_>>();

            Ok(Box::pin(stream::iter(events.into_iter().map(Ok))))
        }

        async fn append(
            &self,
            _events: Vec<Event>,
            _condition: Option<AppendCondition>,
        ) -> Result<Vec<SequencedEvent>, Error<Self::Error>> {
            Ok(Vec::new())
        }
    }

    #[derive(Default)]
    struct FakeProjection {
        applied: AtomicU64,
        reset: AtomicU64,
    }

    impl Projection for FakeProjection {
        fn name(&self) -> &'static str {
            "test.view"
        }

        fn query(&self) -> Query {
            Query::all()
        }

        fn apply<'a>(
            &'a self,
            _event: &'a SequencedEvent,
        ) -> ProjectionFuture<'a, Result<(), ProjectionError>> {
            self.applied.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }

        fn reset(&self) -> ProjectionFuture<'_, Result<(), ProjectionError>> {
            self.reset.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }
    }

    #[derive(Clone, Default)]
    struct FakeCheckpointStore {
        position: Arc<AtomicU64>,
        resets: Arc<AtomicU64>,
    }

    impl CheckpointStore for FakeCheckpointStore {
        type Error = std::io::Error;

        async fn load(&self, _view: &str) -> Result<Option<SequencePosition>, Self::Error> {
            let position = self.position.load(Ordering::SeqCst);
            Ok((position > 0).then_some(position))
        }

        async fn save(&self, _view: &str, position: SequencePosition) -> Result<(), Self::Error> {
            self.position.store(position, Ordering::SeqCst);
            Ok(())
        }

        async fn reset(&self, _view: &str) -> Result<(), Self::Error> {
            self.resets.fetch_add(1, Ordering::SeqCst);
            self.position.store(0, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn supervisor_applies_events_and_resets_checkpoints()
    -> Result<(), Box<dyn std::error::Error>> {
        let event_store = TestEventStore {
            events: Arc::new(vec![SequencedEvent {
                event: Event::new("test.event", serde_json::Value::Null, vec![]),
                position: 1,
            }]),
        };

        let projection = Arc::new(FakeProjection::default());
        let checkpoints = FakeCheckpointStore::default();
        let supervisor = ProjectionSupervisor::new(
            event_store,
            checkpoints.clone(),
            vec![projection.clone()],
            WorkerConfig::new(CatchupConfig::from_interval(
                std::time::Duration::from_secs(60),
            )),
        );

        let (wake_tx, wake_rx) = broadcast::channel(1);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let task = tokio::spawn(supervisor.run(wake_rx, shutdown_rx));

        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while projection.applied.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await?;

        let _ = wake_tx.send(());
        let _ = shutdown_tx.send(true);
        tokio::time::timeout(std::time::Duration::from_secs(1), task).await???;
        assert_eq!(checkpoints.resets.load(Ordering::SeqCst), 0);
        Ok(())
    }

    #[tokio::test]
    async fn rebuild_resets_projection_and_checkpoint() -> Result<(), Box<dyn std::error::Error>> {
        let event_store = TestEventStore::default();
        let projection = Arc::new(FakeProjection::default());
        let checkpoints = FakeCheckpointStore::default();
        let supervisor = ProjectionSupervisor::new(
            event_store,
            checkpoints.clone(),
            vec![projection.clone()],
            WorkerConfig::default(),
        );

        supervisor.rebuild("test.view").await?;
        assert_eq!(projection.reset.load(Ordering::SeqCst), 1);
        assert_eq!(checkpoints.resets.load(Ordering::SeqCst), 1);
        Ok(())
    }
}
