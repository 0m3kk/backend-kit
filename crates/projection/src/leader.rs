use std::time::Duration;

use sqlx::PgPool;
use sqlx::pool::PoolConnection;
use sqlx::postgres::Postgres;
use thiserror::Error;
use tokio::sync::watch;
use tokio::time::sleep;

/// Errors produced while acquiring or releasing the worker leader lock.
#[derive(Debug, Error)]
pub enum LeaderError {
    /// PostgreSQL rejected the lock query or the pool failed.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// Holds the PostgreSQL advisory lock that elects one active worker.
///
/// The lock is session-level, so the guard keeps a dedicated pooled
/// connection alive for as long as it is held. Dropping the connection (for
/// example after a network failure) releases the lock and lets a standby take
/// over.
pub struct LeaderGuard {
    conn: Option<PoolConnection<Postgres>>,
    lock_id: i64,
}

impl LeaderGuard {
    /// Attempts to acquire the lock without waiting.
    ///
    /// Returns `None` when another session holds the lock or when `lock_id` is
    /// not positive (leader election disabled).
    pub async fn try_acquire(pool: &PgPool, lock_id: i64) -> Result<Option<Self>, LeaderError> {
        if lock_id <= 0 {
            return Ok(None);
        }

        let mut conn = pool.acquire().await?;
        let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
            .bind(lock_id)
            .fetch_one(&mut *conn)
            .await?;

        if acquired {
            Ok(Some(Self {
                conn: Some(conn),
                lock_id,
            }))
        } else {
            Ok(None)
        }
    }

    /// Waits until the lock is acquired or shutdown is requested.
    ///
    /// A non-positive `lock_id` disables leader election and returns `None`
    /// immediately. Transient database errors are logged and retried.
    pub async fn acquire(
        pool: &PgPool,
        lock_id: i64,
        retry_interval: Duration,
        shutdown: &mut watch::Receiver<bool>,
    ) -> Result<Option<Self>, LeaderError> {
        if lock_id <= 0 {
            return Ok(None);
        }

        let retry_interval = if retry_interval.is_zero() {
            Duration::from_secs(1)
        } else {
            retry_interval
        };

        loop {
            if *shutdown.borrow() {
                return Ok(None);
            }

            match Self::try_acquire(pool, lock_id).await {
                Ok(Some(guard)) => {
                    tracing::info!(lock_id, "acquired worker leader lock");
                    return Ok(Some(guard));
                }
                Ok(None) => {
                    tracing::info!(
                        lock_id,
                        "worker leader lock held by another instance; standing by"
                    );
                }
                Err(error) => {
                    tracing::error!(%error, "failed to check worker leader lock");
                }
            }

            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        return Ok(None);
                    }
                }
                _ = sleep(retry_interval) => {}
            }
        }
    }

    /// Releases the advisory lock.
    pub async fn release(mut self) {
        let Some(mut conn) = self.conn.take() else {
            return;
        };

        match sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(self.lock_id)
            .execute(&mut *conn)
            .await
        {
            Ok(_) => tracing::info!(lock_id = self.lock_id, "released worker leader lock"),
            Err(error) => {
                tracing::warn!(%error, lock_id = self.lock_id, "failed to release worker leader lock");
                // Do not return a session that may still hold the lock to the pool.
                drop(conn.detach());
            }
        }
    }
}

impl Drop for LeaderGuard {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            // Returning the connection to the pool would keep the session-level
            // advisory lock alive. Detaching closes the session instead.
            drop(conn.detach());
        }
    }
}
