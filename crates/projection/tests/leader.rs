//! Leader election tests for the PostgreSQL advisory lock guard.

use std::error::Error;

use projection::LeaderGuard;
use sqlx::PgPool;

#[sqlx::test]
async fn leader_lock_is_exclusive(pool: PgPool) -> Result<(), Box<dyn Error>> {
    let lock_id = 42;

    let first = LeaderGuard::try_acquire(&pool, lock_id).await?;
    assert!(first.is_some());

    let second = LeaderGuard::try_acquire(&pool, lock_id).await?;
    assert!(second.is_none());

    if let Some(guard) = first {
        guard.release().await;
    }

    let third = LeaderGuard::try_acquire(&pool, lock_id).await?;
    assert!(third.is_some());

    if let Some(guard) = third {
        guard.release().await;
    }

    Ok(())
}

#[sqlx::test]
async fn non_positive_lock_disables_leader_election(pool: PgPool) -> Result<(), Box<dyn Error>> {
    assert!(LeaderGuard::try_acquire(&pool, 0).await?.is_none());
    Ok(())
}
