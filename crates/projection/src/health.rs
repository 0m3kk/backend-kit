//! Shared health state for projection worker loops.

use std::collections::BTreeMap;
use std::sync::Arc;

use event_store::SequencePosition;
use serde::Serialize;
use tokio::sync::RwLock;

/// Consecutive failures after which a projection is considered not ready.
///
/// A single transient database error must not pull a pod out of rotation, but a
/// projection that keeps failing is not making progress and should be
/// restarted or inspected.
pub const UNHEALTHY_AFTER_ERRORS: u32 = 3;

/// Last observed state of one projection loop.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectionStatus {
    /// Stable projection name.
    pub name: String,
    /// Highest event position applied successfully.
    pub last_position: Option<SequencePosition>,
    /// Number of events applied by this process.
    pub processed: u64,
    /// Age of the oldest event in the last applied batch, in seconds.
    pub lag_seconds: Option<f64>,
    /// Number of consecutive failed catch-up attempts.
    pub consecutive_errors: u32,
    /// Last error reported by the loop, if any.
    pub last_error: Option<String>,
}

impl ProjectionStatus {
    fn new(name: String) -> Self {
        Self {
            name,
            last_position: None,
            processed: 0,
            lag_seconds: None,
            consecutive_errors: 0,
            last_error: None,
        }
    }

    /// Returns `true` when the last catch-up attempt succeeded.
    pub fn is_healthy(&self) -> bool {
        self.last_error.is_none()
    }

    /// Returns `true` when the projection has failed less than
    /// [`UNHEALTHY_AFTER_ERRORS`] times in a row.
    pub fn is_ready(&self) -> bool {
        self.consecutive_errors < UNHEALTHY_AFTER_ERRORS
    }
}

/// Health snapshot shared by the supervisor and the worker HTTP server.
#[derive(Clone, Default)]
pub struct WorkerHealth {
    statuses: Arc<RwLock<BTreeMap<String, ProjectionStatus>>>,
}

impl WorkerHealth {
    /// Creates health entries for the given projection names.
    pub fn new(names: impl IntoIterator<Item = String>) -> Self {
        let statuses = names
            .into_iter()
            .map(|name| (name.clone(), ProjectionStatus::new(name)))
            .collect();

        Self {
            statuses: Arc::new(RwLock::new(statuses)),
        }
    }

    /// Records a successful catch-up attempt.
    pub async fn record_success(
        &self,
        name: &str,
        position: SequencePosition,
        processed: u64,
        lag_seconds: Option<f64>,
    ) {
        let mut statuses = self.statuses.write().await;
        let status = statuses
            .entry(name.to_string())
            .or_insert_with(|| ProjectionStatus::new(name.to_string()));

        if processed > 0 || position > 0 {
            status.last_position = Some(position);
        }
        status.processed = status.processed.saturating_add(processed);
        status.lag_seconds = Some(lag_seconds.unwrap_or(0.0));
        status.consecutive_errors = 0;
        status.last_error = None;
    }

    /// Records a failed catch-up attempt.
    pub async fn record_error(&self, name: &str, error: &str) {
        let mut statuses = self.statuses.write().await;
        let status = statuses
            .entry(name.to_string())
            .or_insert_with(|| ProjectionStatus::new(name.to_string()));

        status.consecutive_errors = status.consecutive_errors.saturating_add(1);
        status.last_error = Some(error.to_string());
    }

    /// Returns a stable snapshot ordered by projection name.
    pub async fn snapshot(&self) -> Vec<ProjectionStatus> {
        self.statuses.read().await.values().cloned().collect()
    }

    /// Returns `true` when every projection's last attempt succeeded.
    pub async fn is_healthy(&self) -> bool {
        self.statuses
            .read()
            .await
            .values()
            .all(ProjectionStatus::is_healthy)
    }

    /// Returns `true` when no projection has failed
    /// [`UNHEALTHY_AFTER_ERRORS`] times in a row.
    pub async fn is_ready(&self) -> bool {
        self.statuses
            .read()
            .await
            .values()
            .all(ProjectionStatus::is_ready)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tracks_success_and_failure_streaks() {
        let health = WorkerHealth::new(["catalog.view".to_string()]);

        health
            .record_success("catalog.view", 10, 5, Some(1.5))
            .await;
        let status = &health.snapshot().await[0];
        assert_eq!(status.last_position, Some(10));
        assert_eq!(status.processed, 5);
        assert_eq!(status.lag_seconds, Some(1.5));
        assert!(status.is_ready());

        health.record_error("catalog.view", "boom").await;
        assert!(!health.is_healthy().await);
        assert!(health.is_ready().await);

        health.record_error("catalog.view", "boom").await;
        health.record_error("catalog.view", "boom").await;
        assert!(!health.is_ready().await);

        health.record_success("catalog.view", 11, 1, None).await;
        assert!(health.is_healthy().await);
        assert!(health.is_ready().await);
        assert_eq!(health.snapshot().await[0].consecutive_errors, 0);
    }
}
