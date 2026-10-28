//! Shared health state for projection worker loops.

use std::collections::BTreeMap;
use std::sync::Arc;

use event_store::SequencePosition;
use tokio::sync::RwLock;

/// Last observed state of one projection loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionStatus {
    /// Stable projection name.
    pub name: String,
    /// Highest event position applied successfully.
    pub last_position: Option<SequencePosition>,
    /// Number of events applied by this process.
    pub processed: u64,
    /// Last error reported by the loop, if any.
    pub last_error: Option<String>,
}

impl ProjectionStatus {
    /// Returns `true` when the loop has not reported an error.
    pub fn is_healthy(&self) -> bool {
        self.last_error.is_none()
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
            .map(|name| {
                (
                    name.clone(),
                    ProjectionStatus {
                        name,
                        last_position: None,
                        processed: 0,
                        last_error: None,
                    },
                )
            })
            .collect();

        Self {
            statuses: Arc::new(RwLock::new(statuses)),
        }
    }

    /// Records a successful batch.
    pub async fn record_success(&self, name: &str, position: SequencePosition, processed: u64) {
        let mut statuses = self.statuses.write().await;
        let status = statuses
            .entry(name.to_string())
            .or_insert_with(|| ProjectionStatus {
                name: name.to_string(),
                last_position: None,
                processed: 0,
                last_error: None,
            });

        if processed > 0 {
            status.last_position = Some(position);
        }
        status.processed = status.processed.saturating_add(processed);
        status.last_error = None;
    }

    /// Records a failed catch-up attempt.
    pub async fn record_error(&self, name: &str, error: &str) {
        let mut statuses = self.statuses.write().await;
        let status = statuses
            .entry(name.to_string())
            .or_insert_with(|| ProjectionStatus {
                name: name.to_string(),
                last_position: None,
                processed: 0,
                last_error: None,
            });

        status.last_error = Some(error.to_string());
    }

    /// Returns a stable snapshot ordered by projection name.
    pub async fn snapshot(&self) -> Vec<ProjectionStatus> {
        self.statuses.read().await.values().cloned().collect()
    }

    /// Returns `true` when every projection reports no error.
    pub async fn is_healthy(&self) -> bool {
        self.statuses
            .read()
            .await
            .values()
            .all(ProjectionStatus::is_healthy)
    }
}
