use crate::errors::AttemptError;
use crate::types::{AttemptPolicy, AttemptStatus};
use async_trait::async_trait;

/// Abstract async trait for tracking failed attempt counters, sliding windows, and lockouts.
#[async_trait]
pub trait AttemptTracker: Send + Sync {
    /// Check status of identifier without recording a new failed attempt.
    async fn check_status(
        &self,
        key: &str,
        policy: &AttemptPolicy,
    ) -> Result<AttemptStatus, AttemptError>;

    /// Record a failed attempt for identifier and return updated status.
    async fn record_failed_attempt(
        &self,
        key: &str,
        policy: &AttemptPolicy,
    ) -> Result<AttemptStatus, AttemptError>;

    /// Record a successful attempt, resetting counter if configured.
    async fn record_success(&self, key: &str) -> Result<(), AttemptError>;

    /// Manually reset/unlock identifier.
    async fn reset(&self, key: &str) -> Result<(), AttemptError>;
}
