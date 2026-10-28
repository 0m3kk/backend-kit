use std::fmt::Display;
use std::future::Future;
use std::time::Duration;

use event_store::EventStore;
use tokio::sync::broadcast;
use tokio::time::{self, MissedTickBehavior};

use crate::checkpoint::CheckpointStore;
use crate::view::View;
use crate::worker::ProjectionWorker;

/// Default delay between catch-up runs.
pub const DEFAULT_CATCHUP_INTERVAL: Duration = Duration::from_secs(5);

/// Default debounce window for coalescing event notifications.
pub const DEFAULT_CATCHUP_DEBOUNCE: Duration = Duration::from_millis(50);

/// Configuration for [`run_catchup_loop`] and
/// [`run_catchup_loop_with_wake`].
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

/// Runs `worker` against `view` until `shutdown` completes.
///
/// The first catch-up runs immediately, then once per configured interval.
/// Failures are logged and the loop continues, so a transient database error
/// does not stop the worker.
pub async fn run_catchup_loop<E, C, V, F>(
    worker: &ProjectionWorker<E, C>,
    view: &V,
    config: CatchupConfig,
    shutdown: F,
) where
    E: EventStore + Send + Sync,
    E::Error: Display,
    C: CheckpointStore + Send + Sync,
    C::Error: Display,
    V: View + Send + Sync,
    V::Error: Display,
    F: Future<Output = ()>,
{
    run_loop(worker, view, config, None, shutdown).await;
}

/// Runs `worker` against `view` until `shutdown` completes, waking immediately
/// when a signal arrives on `wake`.
///
/// Signals are hints, not data: every run reads from the checkpoint. Bursts are
/// coalesced by the configured debounce, and the polling ticker remains the
/// safety net for lost notifications.
pub async fn run_catchup_loop_with_wake<E, C, V, F>(
    worker: &ProjectionWorker<E, C>,
    view: &V,
    config: CatchupConfig,
    wake: broadcast::Receiver<()>,
    shutdown: F,
) where
    E: EventStore + Send + Sync,
    E::Error: Display,
    C: CheckpointStore + Send + Sync,
    C::Error: Display,
    V: View + Send + Sync,
    V::Error: Display,
    F: Future<Output = ()>,
{
    run_loop(worker, view, config, Some(wake), shutdown).await;
}

async fn run_loop<E, C, V, F>(
    worker: &ProjectionWorker<E, C>,
    view: &V,
    config: CatchupConfig,
    mut wake: Option<broadcast::Receiver<()>>,
    shutdown: F,
) where
    E: EventStore + Send + Sync,
    E::Error: Display,
    C: CheckpointStore + Send + Sync,
    C::Error: Display,
    V: View + Send + Sync,
    V::Error: Display,
    F: Future<Output = ()>,
{
    let view_name = view.name();
    let mut ticker = time::interval(config.interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    tracing::info!(
        view = view_name,
        interval = ?config.interval,
        debounce = ?config.debounce,
        "projection worker started"
    );

    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                catchup_once(worker, view, view_name).await;
            }
            wake_result = async {
                match &mut wake {
                    Some(receiver) => receiver.recv().await,
                    None => std::future::pending::<Result<(), broadcast::error::RecvError>>().await,
                }
            } => {
                match wake_result {
                    Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {
                        if !config.debounce.is_zero() {
                            time::sleep(config.debounce).await;
                        }
                        catchup_once(worker, view, view_name).await;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        wake = None;
                    }
                }
            }
            _ = &mut shutdown => break,
        }
    }

    tracing::info!(view = view_name, "projection worker stopped");
}

async fn catchup_once<E, C, V>(worker: &ProjectionWorker<E, C>, view: &V, view_name: &str)
where
    E: EventStore + Send + Sync,
    E::Error: Display,
    C: CheckpointStore + Send + Sync,
    C::Error: Display,
    V: View + Send + Sync,
    V::Error: Display,
{
    match worker.catchup(view).await {
        Ok(report) if report.processed > 0 => {
            tracing::info!(
                view = view_name,
                processed = report.processed,
                last_position = report.last_position,
                "projection caught up"
            );
        }
        Ok(_) => {}
        Err(error) => {
            tracing::error!(view = view_name, %error, "projection catch-up failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use event_store::{MockEventStore, Query, SequencePosition, SequencedEvent};
    use futures_util::stream;
    use tokio::sync::watch;

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
        loads: Arc<AtomicU64>,
    }

    impl CheckpointStore for FakeCheckpointStore {
        type Error = std::io::Error;

        async fn load(&self, _view: &str) -> Result<Option<SequencePosition>, Self::Error> {
            self.loads.fetch_add(1, Ordering::SeqCst);
            let position = self.position.load(Ordering::SeqCst);
            Ok((position > 0).then_some(position))
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

    #[test]
    fn parses_interval_override() {
        assert_eq!(parse_interval(Some("1")), Duration::from_secs(1));
        assert_eq!(parse_interval(Some("60")), Duration::from_secs(60));
        assert_eq!(parse_interval(Some("0")), DEFAULT_CATCHUP_INTERVAL);
        assert_eq!(parse_interval(Some("invalid")), DEFAULT_CATCHUP_INTERVAL);
        assert_eq!(parse_interval(None), DEFAULT_CATCHUP_INTERVAL);
    }

    #[test]
    fn zero_interval_falls_back_to_default() {
        assert_eq!(
            CatchupConfig::from_interval(Duration::ZERO).interval(),
            DEFAULT_CATCHUP_INTERVAL
        );
    }

    #[test]
    fn debounce_defaults_and_override() {
        assert_eq!(
            CatchupConfig::default().debounce(),
            DEFAULT_CATCHUP_DEBOUNCE
        );
        assert!(
            CatchupConfig::default()
                .with_debounce(Duration::ZERO)
                .debounce()
                .is_zero()
        );
    }

    #[tokio::test]
    async fn stops_when_shutdown_completes() -> Result<(), Box<dyn std::error::Error>> {
        let mut event_store = MockEventStore::new();
        event_store
            .expect_read()
            .returning(|_, _| Ok(Box::pin(stream::empty())));

        let worker = ProjectionWorker::new(event_store, FakeCheckpointStore::default());
        let view = FakeView::default();

        tokio::time::timeout(
            Duration::from_secs(1),
            run_catchup_loop(
                &worker,
                &view,
                CatchupConfig::from_interval(Duration::from_secs(60)),
                std::future::ready(()),
            ),
        )
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn wake_signal_triggers_catchup() -> Result<(), Box<dyn std::error::Error>> {
        let mut event_store = MockEventStore::new();
        event_store
            .expect_read()
            .returning(|_, _| Ok(Box::pin(stream::empty())));

        let checkpoints = FakeCheckpointStore::default();
        let worker = ProjectionWorker::new(event_store, checkpoints.clone());
        let view = FakeView::default();
        let (wake_tx, wake_rx) = broadcast::channel(1);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let task = tokio::spawn(async move {
            let shutdown = async move {
                let mut rx = shutdown_rx;
                let _ = rx.changed().await;
            };
            run_catchup_loop_with_wake(
                &worker,
                &view,
                CatchupConfig::from_interval(Duration::from_secs(60)).with_debounce(Duration::ZERO),
                wake_rx,
                shutdown,
            )
            .await;
        });

        // The first tick is immediate. Wait for it, then a notification must
        // trigger another catch-up without waiting for the 60-second ticker.
        wait_for_loads(&checkpoints, 0).await?;
        let before_wake = checkpoints.loads.load(Ordering::SeqCst);
        let _ = wake_tx.send(());
        wait_for_loads(&checkpoints, before_wake).await?;

        let _ = shutdown_tx.send(true);
        tokio::time::timeout(Duration::from_secs(1), task).await??;

        Ok(())
    }

    async fn wait_for_loads(
        checkpoints: &FakeCheckpointStore,
        previous: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        tokio::time::timeout(Duration::from_secs(1), async {
            while checkpoints.loads.load(Ordering::SeqCst) <= previous {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await?;

        Ok(())
    }
}
