use crate::errors::AttemptError;
use crate::kv::KvAttemptTracker;
use crate::tracker::AttemptTracker;
use crate::types::{AttemptPolicy, AttemptStatus};
use kv_store::{KvStore, KvStoreTx};

/// High-level manager pairing an `AttemptPolicy` with an `AttemptTracker`.
#[derive(Debug, Clone)]
pub struct AttemptManager<T: AttemptTracker> {
    pub policy: AttemptPolicy,
    pub tracker: T,
}

impl<T: AttemptTracker> AttemptManager<T> {
    pub fn new(policy: AttemptPolicy, tracker: T) -> Self {
        Self { policy, tracker }
    }

    pub fn with_tracker(policy: AttemptPolicy, tracker: T) -> Self {
        Self::new(policy, tracker)
    }

    /// Check if key is allowed to attempt operation, returning `Err(AttemptError::MaxAttemptsExceeded)` if locked.
    pub async fn check_attempt(&self, key: &str) -> Result<AttemptStatus, AttemptError> {
        let status = self.tracker.check_status(key, &self.policy).await?;
        into_result(status)
    }

    /// Record a failed attempt for key, returning `Err(AttemptError::MaxAttemptsExceeded)` if lockout limit reached.
    pub async fn record_failed_attempt(&self, key: &str) -> Result<AttemptStatus, AttemptError> {
        let status = self
            .tracker
            .record_failed_attempt(key, &self.policy)
            .await?;
        into_result(status)
    }

    /// Record successful attempt, resetting counter if configured.
    pub async fn record_success(&self, key: &str) -> Result<(), AttemptError> {
        if self.policy.reset_on_success {
            self.tracker.record_success(key).await?;
        }
        Ok(())
    }

    /// Unlock/reset key.
    pub async fn unlock(&self, key: &str) -> Result<(), AttemptError> {
        self.tracker.reset(key).await
    }
}

// ---------------------------------------------------------------------------
// Transactional methods (available when tracker is KvAttemptTracker<S: KvStoreTx<Conn>>)
// ---------------------------------------------------------------------------

impl<S: KvStore> AttemptManager<KvAttemptTracker<S>> {
    /// Check if key is allowed within an external database transaction.
    pub async fn check_attempt_tx<Conn: Send>(
        &self,
        conn: &mut Conn,
        key: &str,
    ) -> Result<AttemptStatus, AttemptError>
    where
        S: KvStoreTx<Conn>,
    {
        let status = self
            .tracker
            .check_status_tx(conn, key, &self.policy)
            .await?;
        into_result(status)
    }

    /// Record a failed attempt within an external database transaction.
    pub async fn record_failed_attempt_tx<Conn: Send>(
        &self,
        conn: &mut Conn,
        key: &str,
    ) -> Result<AttemptStatus, AttemptError>
    where
        S: KvStoreTx<Conn>,
    {
        let status = self
            .tracker
            .record_failed_attempt_tx(conn, key, &self.policy)
            .await?;
        into_result(status)
    }

    /// Record a successful attempt within an external database transaction.
    pub async fn record_success_tx<Conn: Send>(
        &self,
        conn: &mut Conn,
        key: &str,
    ) -> Result<(), AttemptError>
    where
        S: KvStoreTx<Conn>,
    {
        if self.policy.reset_on_success {
            self.tracker.record_success_tx(conn, key).await?;
        }
        Ok(())
    }

    /// Unlock/reset key within an external database transaction.
    pub async fn unlock_tx<Conn: Send>(
        &self,
        conn: &mut Conn,
        key: &str,
    ) -> Result<(), AttemptError>
    where
        S: KvStoreTx<Conn>,
    {
        self.tracker.reset_tx(conn, key).await
    }
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn into_result(status: AttemptStatus) -> Result<AttemptStatus, AttemptError> {
    if let AttemptStatus::LockedOut { retry_after, .. } = status {
        Err(AttemptError::MaxAttemptsExceeded {
            retry_after_secs: retry_after.as_secs().max(1),
        })
    } else {
        Ok(status)
    }
}
