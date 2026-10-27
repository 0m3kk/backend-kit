use std::fmt::Display;
use std::future::Future;
use std::time::Duration;

use event_store::EventStore;
use tokio::time::{self, MissedTickBehavior};

use crate::checkpoint::CheckpointStore;
use crate::view::View;
use crate::worker::ProjectionWorker;

/// Default delay between catch-up runs.
pub const DEFAULT_CATCHUP_INTERVAL: Duration = Duration::from_secs(5);

/// Configuration for [`run_catchup_loop`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatchupConfig {
    interval: Duration,
}

impl Default for CatchupConfig {
    fn default() -> Self {
        Self {
            interval: DEFAULT_CATCHUP_INTERVAL,
        }
    }
}

impl CatchupConfig {
    /// Creates a configuration with the given interval.
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
        }
    }

    /// Creates a configuration from an environment variable holding seconds.
    ///
    /// Missing, invalid, and zero values fall back to the default interval.
    pub fn from_env(env_var: &str) -> Self {
        Self::from_interval(parse_interval(std::env::var(env_var).ok().as_deref()))
    }

    /// Returns the configured interval.
    pub fn interval(&self) -> Duration {
        self.interval
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
    let view_name = view.name();
    let mut ticker = time::interval(config.interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    tracing::info!(
        view = view_name,
        interval = ?config.interval,
        "projection worker started"
    );

    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
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
            _ = &mut shutdown => break,
        }
    }

    tracing::info!(view = view_name, "projection worker stopped");
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use event_store::{MockEventStore, Query, SequencePosition, SequencedEvent};
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
}
