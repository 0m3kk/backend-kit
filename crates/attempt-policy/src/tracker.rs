use crate::errors::AttemptError;
use crate::types::{AttemptPolicy, AttemptStatus};
use async_trait::async_trait;
use std::sync::Arc;

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

#[async_trait]
impl<T: AttemptTracker + ?Sized> AttemptTracker for Arc<T> {
    async fn check_status(
        &self,
        key: &str,
        policy: &AttemptPolicy,
    ) -> Result<AttemptStatus, AttemptError> {
        (**self).check_status(key, policy).await
    }

    async fn record_failed_attempt(
        &self,
        key: &str,
        policy: &AttemptPolicy,
    ) -> Result<AttemptStatus, AttemptError> {
        (**self).record_failed_attempt(key, policy).await
    }

    async fn record_success(&self, key: &str) -> Result<(), AttemptError> {
        (**self).record_success(key).await
    }

    async fn reset(&self, key: &str) -> Result<(), AttemptError> {
        (**self).reset(key).await
    }
}

/// Transactional attempt tracker operations.
///
/// `Conn` represents the connection or transaction handle type. The caller owns the
/// transaction lifecycle — the tracker only executes operations through the provided handle.
#[async_trait]
pub trait AttemptTrackerTx<Conn: Send>: Send + Sync {
    /// Check status of identifier within an external transaction.
    async fn check_status_tx(
        &self,
        conn: &mut Conn,
        key: &str,
        policy: &AttemptPolicy,
    ) -> Result<AttemptStatus, AttemptError>;

    /// Record a failed attempt within an external transaction.
    async fn record_failed_attempt_tx(
        &self,
        conn: &mut Conn,
        key: &str,
        policy: &AttemptPolicy,
    ) -> Result<AttemptStatus, AttemptError>;

    /// Record a successful attempt within an external transaction.
    async fn record_success_tx(&self, conn: &mut Conn, key: &str) -> Result<(), AttemptError>;

    /// Reset/unlock identifier within an external transaction.
    async fn reset_tx(&self, conn: &mut Conn, key: &str) -> Result<(), AttemptError>;
}

#[async_trait]
impl<T: AttemptTrackerTx<Conn> + ?Sized, Conn: Send> AttemptTrackerTx<Conn> for Arc<T> {
    async fn check_status_tx(
        &self,
        conn: &mut Conn,
        key: &str,
        policy: &AttemptPolicy,
    ) -> Result<AttemptStatus, AttemptError> {
        (**self).check_status_tx(conn, key, policy).await
    }

    async fn record_failed_attempt_tx(
        &self,
        conn: &mut Conn,
        key: &str,
        policy: &AttemptPolicy,
    ) -> Result<AttemptStatus, AttemptError> {
        (**self).record_failed_attempt_tx(conn, key, policy).await
    }

    async fn record_success_tx(&self, conn: &mut Conn, key: &str) -> Result<(), AttemptError> {
        (**self).record_success_tx(conn, key).await
    }

    async fn reset_tx(&self, conn: &mut Conn, key: &str) -> Result<(), AttemptError> {
        (**self).reset_tx(conn, key).await
    }
}
