//! Checkpoint transaction tests.

use std::error::Error;

use projection::{CheckpointError, CheckpointStore, PostgresCheckpointStore, save_in_transaction};
use sqlx::PgPool;

#[sqlx::test]
async fn checkpoint_rolls_back_with_the_transaction(pool: PgPool) -> Result<(), Box<dyn Error>> {
    PostgresCheckpointStore::migrate(&pool).await?;
    let store = PostgresCheckpointStore::new(pool.clone());

    let mut tx = pool.begin().await?;
    save_in_transaction(&mut tx, "test.view", 7).await?;
    tx.rollback().await?;

    assert_eq!(store.load("test.view").await?, None);
    Ok(())
}

#[sqlx::test]
async fn checkpoint_never_moves_backwards(pool: PgPool) -> Result<(), Box<dyn Error>> {
    PostgresCheckpointStore::migrate(&pool).await?;
    let store = PostgresCheckpointStore::new(pool.clone());
    store.save("test.view", 10).await?;

    let mut tx = pool.begin().await?;
    save_in_transaction(&mut tx, "test.view", 4).await?;
    tx.commit().await?;

    assert_eq!(store.load("test.view").await?, Some(10));
    Ok(())
}

#[sqlx::test]
async fn checkpoint_commits_with_the_transaction(pool: PgPool) -> Result<(), Box<dyn Error>> {
    PostgresCheckpointStore::migrate(&pool).await?;
    let store = PostgresCheckpointStore::new(pool.clone());

    let mut tx = pool.begin().await?;
    save_in_transaction(&mut tx, "test.view", 3).await?;
    tx.commit().await?;

    assert_eq!(store.load("test.view").await?, Some(3));
    Ok(())
}

#[sqlx::test]
async fn positions_outside_bigint_are_rejected(pool: PgPool) -> Result<(), Box<dyn Error>> {
    PostgresCheckpointStore::migrate(&pool).await?;

    let mut tx = pool.begin().await?;
    let result = save_in_transaction(&mut tx, "test.view", u64::MAX).await;
    tx.rollback().await?;

    assert!(matches!(result, Err(CheckpointError::PositionOverflow)));
    Ok(())
}
