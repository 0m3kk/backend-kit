//! Multi-projection supervisor: one catch-up loop per view.

use std::fmt::Display;
use std::sync::Arc;
use std::time::Duration;

use event_store::{EventStore, SequencePosition};
use futures_util::StreamExt;
use thiserror::Error;
use tokio::sync::{broadcast, watch};
use tokio::task::JoinSet;
use tokio::time::{self, MissedTickBehavior};

use crate::checkpoint::CheckpointStore;
use crate::health::WorkerHealth;
use crate::metrics::{ProjectionMetrics, event_age_seconds};
use crate::projection::{Projection, ProjectionError};

/// Default delay between catch-up runs.
pub const DEFAULT_CATCHUP_INTERVAL: Duration = Duration::from_secs(5);

/// Default debounce window for coalescing event notifications.
pub const DEFAULT_CATCHUP_DEBOUNCE: Duration = Duration::from_millis(50);

/// Configuration for the catch-up loops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatchupConfig {
    interval: Duration,
    debounce: Duration,
}

impl Default for CatchupConfig {
    fn default() -> Self {
        Self {
            interval: DEFAULT_CATCHUP_INTERVAL,
            debounce: DEFAULT_CATCHUP_DEBOUNCE,
        }
    }
}

impl CatchupConfig {
    /// Creates a configuration with the given polling interval.
    ///
    /// A zero interval falls back to [`DEFAULT_CATCHUP_INTERVAL`] because
    /// `tokio::time::interval` panics on zero.
    pub fn from_interval(interval: Duration) -> Self {
        Self {
            interval: if interval.is_zero() {
                DEFAULT_CATCHUP_INTERVAL
            } else {
                interval
            },
            debounce: DEFAULT_CATCHUP_DEBOUNCE,
        }
    }

    /// Creates a configuration from an environment variable holding seconds.
    ///
    /// Missing, invalid, and zero values fall back to the default interval.
    pub fn from_env(env_var: &str) -> Self {
        Self::from_interval(parse_interval(std::env::var(env_var).ok().as_deref()))
    }

    /// Sets the debounce window applied to wake signals.
    ///
    /// A zero debounce catches up immediately. The default is 50 ms.
    #[must_use]
    pub fn with_debounce(mut self, debounce: Duration) -> Self {
        self.debounce = debounce;
        self
    }

    /// Returns the polling interval.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Returns the notification debounce window.
    pub fn debounce(&self) -> Duration {
        self.debounce
    }
}

fn parse_interval(value: Option<&str>) -> Duration {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_CATCHUP_INTERVAL)
}

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

/// Result of one catch-up pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CatchupReport {
    /// Number of events applied during this pass.
    pub processed: u64,
    /// Highest event position applied.
    pub last_position: SequencePosition,
    /// `true` when the pass drained every event matching the query.
    pub caught_up: bool,
    /// Age of the oldest event in this batch, in seconds.
    pub lag_seconds: Option<f64>,
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
    metrics: ProjectionMetrics,
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

        Self::with_health(event_store, checkpoints, projections, config, health)
    }

    /// Creates a supervisor that reports into an existing health snapshot.
    ///
    /// Use this when the probe server must be available before the worker
    /// becomes leader and starts the supervisor.
    pub fn with_health(
        event_store: E,
        checkpoints: C,
        projections: Vec<Arc<dyn Projection>>,
        config: WorkerConfig,
        health: WorkerHealth,
    ) -> Self {
        Self {
            event_store,
            checkpoints,
            projections,
            config,
            health,
            metrics: ProjectionMetrics::new(),
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
    E: EventStore + Send + Sync,
    E::Error: Display,
    C: CheckpointStore + Send + Sync,
    C::Error: Display,
{
    /// Runs one catch-up pass for `name`.
    ///
    /// The supervisor keeps calling this until a pass reports
    /// [`CatchupReport::caught_up`]. This method does not update health or
    /// metrics; use it for tests and one-shot drains.
    pub async fn catchup_once(&self, name: &str) -> Result<CatchupReport, SupervisorError> {
        let projection = self
            .projections
            .iter()
            .find(|projection| projection.name() == name)
            .ok_or_else(|| SupervisorError::UnknownProjection(name.to_string()))?;

        catchup_pass(
            &self.event_store,
            &self.checkpoints,
            projection.as_ref(),
            self.config.batch_size(),
        )
        .await
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
            metrics,
        } = self;

        if projections.is_empty() {
            let mut shutdown = shutdown;
            if !*shutdown.borrow() {
                let _ = shutdown.changed().await;
            }
            return Ok(());
        }

        let mut tasks = JoinSet::new();

        for projection in projections {
            let event_store = event_store.clone();
            let checkpoints = checkpoints.clone();
            let health = health.clone();
            let metrics = metrics.clone();
            let projection_wake = wake.resubscribe();
            let projection_shutdown = shutdown.clone();

            tasks.spawn(async move {
                run_projection_loop(
                    LoopState {
                        event_store,
                        checkpoints,
                        projection,
                        config,
                        health,
                        metrics,
                    },
                    projection_wake,
                    projection_shutdown,
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

struct LoopState<E, C> {
    event_store: E,
    checkpoints: C,
    projection: Arc<dyn Projection>,
    config: WorkerConfig,
    health: WorkerHealth,
    metrics: ProjectionMetrics,
}

async fn run_projection_loop<E, C>(
    state: LoopState<E, C>,
    wake: broadcast::Receiver<()>,
    mut shutdown: watch::Receiver<bool>,
) where
    E: EventStore + Send + Sync,
    E::Error: Display,
    C: CheckpointStore + Send + Sync,
    C::Error: Display,
{
    let name = state.projection.name();
    let mut wake = Some(wake);
    let mut ticker = time::interval(state.config.catchup.interval());
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    tracing::info!(
        view = name,
        interval = ?state.config.catchup.interval(),
        batch_size = state.config.batch_size(),
        "projection supervisor loop started"
    );

    'shutdown: loop {
        tokio::select! {
            _ = ticker.tick() => {}
            wake_result = async {
                match &mut wake {
                    Some(receiver) => receiver.recv().await,
                    None => std::future::pending::<Result<(), broadcast::error::RecvError>>().await,
                }
            } => {
                match wake_result {
                    Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {
                        if !state.config.catchup.debounce().is_zero() {
                            time::sleep(state.config.catchup.debounce()).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        // Keep polling; notifications are only hints.
                        wake = None;
                    }
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break 'shutdown;
                }
                continue 'shutdown;
            }
        }

        // Drain full batches before going back to sleep. Shutdown is checked
        // between batches, so an in-flight batch can commit its view writes and
        // checkpoint together instead of being cancelled half-way.
        loop {
            if *shutdown.borrow() {
                break 'shutdown;
            }

            match state.catchup_and_record().await {
                Some(report) if !report.caught_up => {}
                _ => break,
            }
        }
    }

    tracing::info!(view = name, "projection supervisor loop stopped");
}

impl<E, C> LoopState<E, C>
where
    E: EventStore + Send + Sync,
    E::Error: Display,
    C: CheckpointStore + Send + Sync,
    C::Error: Display,
{
    async fn catchup_and_record(&self) -> Option<CatchupReport> {
        match catchup_pass(
            &self.event_store,
            &self.checkpoints,
            self.projection.as_ref(),
            self.config.batch_size(),
        )
        .await
        {
            Ok(report) => {
                self.health
                    .record_success(
                        self.projection.name(),
                        report.last_position,
                        report.processed,
                        report.lag_seconds,
                    )
                    .await;
                self.metrics.record_success(
                    self.projection.name(),
                    report.last_position,
                    report.processed,
                    report.lag_seconds,
                );

                if report.processed > 0 {
                    tracing::info!(
                        view = self.projection.name(),
                        processed = report.processed,
                        last_position = report.last_position,
                        "projection caught up"
                    );
                }

                Some(report)
            }
            Err(error) => {
                self.health
                    .record_error(self.projection.name(), &error.to_string())
                    .await;
                self.metrics.record_error(self.projection.name());
                tracing::error!(
                    view = self.projection.name(),
                    %error,
                    "projection catch-up failed"
                );
                None
            }
        }
    }
}

async fn catchup_pass<E, C>(
    event_store: &E,
    checkpoints: &C,
    projection: &dyn Projection,
    batch_size: usize,
) -> Result<CatchupReport, SupervisorError>
where
    E: EventStore + Send + Sync,
    E::Error: Display,
    C: CheckpointStore + Send + Sync,
    C::Error: Display,
{
    let name = projection.name();
    let last_position = checkpoints
        .load(name)
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

    let caught_up = events.len() < batch_size;

    if events.is_empty() {
        return Ok(CatchupReport {
            processed: 0,
            last_position: highest,
            caught_up: true,
            lag_seconds: None,
        });
    }

    // The first event is the oldest unprocessed event, so its age is the
    // current projection lag.
    let lag_seconds = event_age_seconds(&events[0]);
    let processed = events.len() as u64;

    projection
        .apply_batch(&events, highest)
        .await
        .map_err(|source| SupervisorError::Projection {
            projection: name.to_string(),
            source,
        })?;

    Ok(CatchupReport {
        processed,
        last_position: highest,
        caught_up,
        lag_seconds,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    use event_store::{
        AppendCondition, Error, Event, EventStore, EventStream, Query, SequencedEvent,
    };
    use futures_util::stream;
    use tokio::sync::Notify;

    use super::*;
    use crate::projection::ProjectionFuture;

    fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
        match mutex.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn sequenced(position: SequencePosition) -> SequencedEvent {
        SequencedEvent {
            event: Event::new("test.event", serde_json::Value::Null, vec![]),
            position,
        }
    }

    #[derive(Clone, Default)]
    struct TestEventStore {
        events: Arc<Vec<SequencedEvent>>,
    }

    impl TestEventStore {
        fn with_events(events: Vec<SequencedEvent>) -> Self {
            Self {
                events: Arc::new(events),
            }
        }
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

    #[derive(Clone, Default)]
    struct FakeCheckpointStore {
        position: Arc<AtomicU64>,
        resets: Arc<AtomicU64>,
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
            self.resets.fetch_add(1, Ordering::SeqCst);
            self.position.store(0, Ordering::SeqCst);
            Ok(())
        }
    }

    #[derive(Clone)]
    struct FakeProjection {
        checkpoints: FakeCheckpointStore,
        poison: Arc<Mutex<BTreeSet<SequencePosition>>>,
        committed: Arc<Mutex<Vec<SequencePosition>>>,
        batches: Arc<AtomicU64>,
        resets: Arc<AtomicU64>,
        release: Option<Arc<Notify>>,
    }

    impl FakeProjection {
        fn new(checkpoints: FakeCheckpointStore) -> Self {
            Self {
                checkpoints,
                poison: Arc::default(),
                committed: Arc::default(),
                batches: Arc::default(),
                resets: Arc::default(),
                release: None,
            }
        }

        fn blocking(checkpoints: FakeCheckpointStore) -> (Self, Arc<Notify>) {
            let release = Arc::new(Notify::new());
            (
                Self {
                    release: Some(release.clone()),
                    ..Self::new(checkpoints)
                },
                release,
            )
        }

        fn committed(&self) -> Vec<SequencePosition> {
            lock(&self.committed).clone()
        }

        fn batches(&self) -> u64 {
            self.batches.load(Ordering::SeqCst)
        }
    }

    impl Projection for FakeProjection {
        fn name(&self) -> &'static str {
            "test.view"
        }

        fn query(&self) -> Query {
            Query::all()
        }

        fn apply_batch<'a>(
            &'a self,
            events: &'a [SequencedEvent],
            checkpoint: SequencePosition,
        ) -> ProjectionFuture<'a, Result<(), ProjectionError>> {
            Box::pin(async move {
                self.batches.fetch_add(1, Ordering::SeqCst);

                if let Some(release) = &self.release {
                    release.notified().await;
                }

                let poisoned = events
                    .iter()
                    .any(|event| lock(&self.poison).contains(&event.position));
                if poisoned {
                    return Err(ProjectionError::UnexpectedEvent("poison".to_string()));
                }

                lock(&self.committed).extend(events.iter().map(|event| event.position));
                self.checkpoints
                    .save(self.name(), checkpoint)
                    .await
                    .map_err(|error| ProjectionError::Store(error.to_string()))?;
                Ok(())
            })
        }

        fn reset(&self) -> ProjectionFuture<'_, Result<(), ProjectionError>> {
            self.resets.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }
    }

    fn supervisor(
        events: Vec<SequencedEvent>,
        projection: &FakeProjection,
        checkpoints: &FakeCheckpointStore,
        batch_size: usize,
    ) -> ProjectionSupervisor<TestEventStore, FakeCheckpointStore> {
        ProjectionSupervisor::new(
            TestEventStore::with_events(events),
            checkpoints.clone(),
            vec![Arc::new(projection.clone())],
            WorkerConfig::default().with_batch_size(batch_size),
        )
    }

    #[tokio::test]
    async fn applies_events_and_advances_the_checkpoint() -> Result<(), Box<dyn std::error::Error>>
    {
        let checkpoints = FakeCheckpointStore::default();
        let projection = FakeProjection::new(checkpoints.clone());
        let supervisor = supervisor(
            vec![sequenced(1), sequenced(2), sequenced(3)],
            &projection,
            &checkpoints,
            10,
        );

        let report = supervisor.catchup_once("test.view").await?;

        assert_eq!(report.processed, 3);
        assert_eq!(report.last_position, 3);
        assert!(report.caught_up);
        assert_eq!(checkpoints.position(), Some(3));
        assert_eq!(projection.committed(), vec![1, 2, 3]);
        Ok(())
    }

    #[tokio::test]
    async fn failed_batch_keeps_the_checkpoint_and_is_retried()
    -> Result<(), Box<dyn std::error::Error>> {
        let checkpoints = FakeCheckpointStore::default();
        let projection = FakeProjection::new(checkpoints.clone());
        lock(&projection.poison).insert(2);
        let supervisor = supervisor(
            vec![sequenced(1), sequenced(2), sequenced(3)],
            &projection,
            &checkpoints,
            10,
        );

        assert!(supervisor.catchup_once("test.view").await.is_err());
        assert_eq!(checkpoints.position(), None);
        assert!(projection.committed().is_empty());

        // A poison event is retried with the whole batch.
        assert!(supervisor.catchup_once("test.view").await.is_err());
        assert_eq!(projection.batches(), 2);
        assert_eq!(checkpoints.position(), None);

        lock(&projection.poison).clear();
        let report = supervisor.catchup_once("test.view").await?;

        assert_eq!(report.processed, 3);
        assert_eq!(checkpoints.position(), Some(3));
        assert_eq!(projection.committed(), vec![1, 2, 3]);
        Ok(())
    }

    #[tokio::test]
    async fn lost_notification_is_covered_by_polling() -> Result<(), Box<dyn std::error::Error>> {
        let checkpoints = FakeCheckpointStore::default();
        let projection = FakeProjection::new(checkpoints.clone());
        let supervisor = ProjectionSupervisor::new(
            TestEventStore::with_events(vec![sequenced(1), sequenced(2), sequenced(3)]),
            checkpoints.clone(),
            vec![Arc::new(projection.clone())],
            WorkerConfig::new(CatchupConfig::from_interval(Duration::from_millis(20))),
        );

        let (wake_tx, wake_rx) = broadcast::channel(1);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let task = tokio::spawn(supervisor.run(wake_rx, shutdown_rx));

        tokio::time::timeout(Duration::from_secs(1), async {
            while checkpoints.position() != Some(3) {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await?;

        // Closing the hint channel must not stop the polling ticker.
        drop(wake_tx);
        tokio::time::sleep(Duration::from_millis(50)).await;

        let _ = shutdown_tx.send(true);
        tokio::time::timeout(Duration::from_secs(1), task).await???;
        Ok(())
    }

    #[tokio::test]
    async fn shutdown_finishes_the_in_flight_batch_and_stops()
    -> Result<(), Box<dyn std::error::Error>> {
        let checkpoints = FakeCheckpointStore::default();
        let (projection, release) = FakeProjection::blocking(checkpoints.clone());
        let supervisor = ProjectionSupervisor::new(
            TestEventStore::with_events(vec![sequenced(1), sequenced(2)]),
            checkpoints.clone(),
            vec![Arc::new(projection.clone())],
            WorkerConfig::new(CatchupConfig::from_interval(Duration::from_secs(60)))
                .with_batch_size(1),
        );

        let (wake_tx, wake_rx) = broadcast::channel(1);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let task = tokio::spawn(supervisor.run(wake_rx, shutdown_rx));

        tokio::time::timeout(Duration::from_secs(1), async {
            while projection.batches() == 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await?;

        let _ = shutdown_tx.send(true);
        release.notify_one();

        tokio::time::timeout(Duration::from_secs(1), task).await???;

        // The in-flight batch committed, but no new batch started.
        assert_eq!(projection.batches(), 1);
        assert_eq!(projection.committed(), vec![1]);
        assert_eq!(checkpoints.position(), Some(1));
        drop(wake_tx);
        Ok(())
    }

    #[tokio::test]
    async fn rebuild_resets_projection_and_checkpoint() -> Result<(), Box<dyn std::error::Error>> {
        let checkpoints = FakeCheckpointStore::default();
        let projection = FakeProjection::new(checkpoints.clone());
        let supervisor = supervisor(Vec::new(), &projection, &checkpoints, 10);

        supervisor.rebuild("test.view").await?;
        assert_eq!(projection.resets.load(Ordering::SeqCst), 1);
        assert_eq!(checkpoints.resets.load(Ordering::SeqCst), 1);
        Ok(())
    }

    #[tokio::test]
    async fn rebuild_rejects_unknown_projection() {
        let checkpoints = FakeCheckpointStore::default();
        let projection = FakeProjection::new(checkpoints.clone());
        let supervisor = supervisor(Vec::new(), &projection, &checkpoints, 10);

        assert!(matches!(
            supervisor.rebuild("unknown.view").await,
            Err(SupervisorError::UnknownProjection(_))
        ));
    }
}
