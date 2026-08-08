use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use thiserror::Error;
use tracing::warn;

/// Transaction isolation levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IsolationLevel {
    /// Default read committed isolation.
    #[default]
    ReadCommitted,
    /// Repeatable read isolation.
    RepeatableRead,
    /// Serializable isolation with strict serializability guarantees.
    Serializable,
}

impl IsolationLevel {
    /// Returns the SQL BEGIN statement corresponding to this isolation level.
    pub fn sql_begin(&self) -> &'static str {
        match self {
            Self::ReadCommitted => "BEGIN ISOLATION LEVEL READ COMMITTED",
            Self::RepeatableRead => "BEGIN ISOLATION LEVEL REPEATABLE READ",
            Self::Serializable => "BEGIN ISOLATION LEVEL SERIALIZABLE",
        }
    }
}

/// Retry policy for transient transaction errors (e.g., PostgreSQL 40001 serialization failure or 40P01 deadlock).
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of attempts.
    pub max_attempts: u32,
    /// Base delay between retries.
    pub initial_delay: Duration,
    /// Maximum delay between retries.
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_delay: Duration::from_millis(5),
            max_delay: Duration::from_millis(200),
        }
    }
}

impl RetryPolicy {
    /// Calculate exponential backoff duration for a given attempt.
    pub fn backoff_for(&self, attempt: u32) -> Duration {
        let exp = attempt.saturating_sub(1);
        let factor = 2u32.saturating_pow(exp);
        let delay = self.initial_delay.saturating_mul(factor);
        delay.min(self.max_delay)
    }
}

/// Errors occurring during transactional unit of work execution.
#[derive(Debug, Error)]
pub enum TransactionError<E> {
    /// Business logic or domain error produced inside the transactional closure.
    #[error("Domain error: {0}")]
    Domain(#[source] E),

    /// Infrastructure or driver-level database error.
    #[error("Database error: {0}")]
    Database(String),

    /// All retry attempts exhausted due to repeated transient serialization conflicts.
    #[error("Transaction retries exhausted after {attempts} attempts: {last_error}")]
    RetriesExhausted { attempts: u32, last_error: String },
}

/// Abstract runner for executing units of work inside a managed transaction lifecycle.
pub trait TransactionRunner: Send + Sync {
    /// Type representing the active Unit of Work handle passed to the closure.
    type Work<'c>
    where
        Self: 'c;

    /// Runs a transactional closure `F` with automatic transaction begin, commit, rollback on error,
    /// and retries on transient serialization failures.
    fn run<'a, F, R, E>(
        &'a self,
        work: F,
    ) -> Pin<Box<dyn Future<Output = Result<R, TransactionError<E>>> + Send + 'a>>
    where
        F: for<'c> FnMut(
                &'c mut Self::Work<'c>,
            ) -> Pin<Box<dyn Future<Output = Result<R, E>> + Send + 'c>>
            + Send
            + 'a,
        R: Send + 'a,
        E: Send + 'a;
}

/// Checks if an error message indicates a transient, retryable transaction failure.
pub fn is_retryable_sql_error(err_msg: &str) -> bool {
    err_msg.contains("40001")
        || err_msg.contains("40P01")
        || err_msg.contains("serialization failure")
        || err_msg.contains("deadlock detected")
}

/// Helper function to sleep for exponential backoff during retry loops.
pub async fn apply_backoff(policy: &RetryPolicy, attempt: u32, err_msg: &str) {
    let delay = policy.backoff_for(attempt);
    warn!(
        attempt = attempt,
        delay_ms = delay.as_millis(),
        error = err_msg,
        "Transient transaction conflict detected; applying backoff retry"
    );
    tokio::time::sleep(delay).await;
}
