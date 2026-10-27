use std::future::Future;

use event_store::SequencePosition;
use sqlx::PgPool;
use thiserror::Error;

/// Stores the last processed event position for a named projection.
pub trait CheckpointStore: Send + Sync {
    /// Error type of the underlying store.
    type Error;

    /// Loads the last processed position, or `None` when the projection has
    /// never run.
    fn load(
        &self,
        view: &str,
    ) -> impl Future<Output = Result<Option<SequencePosition>, Self::Error>> + Send;

    /// Persists the highest processed position.
    fn save(
        &self,
        view: &str,
        position: SequencePosition,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/// Errors produced by the PostgreSQL checkpoint store.
#[derive(Debug, Error)]
pub enum CheckpointError {
    /// PostgreSQL rejected the checkpoint query.
    #[error(transparent)]
    Database(#[from] sqlx::Error),

    /// An event position cannot be represented as a PostgreSQL `BIGINT`.
    #[error("event position does not fit into a BIGINT")]
    PositionOverflow,
}

/// PostgreSQL implementation backed by the `projection_checkpoints` table.
#[derive(Clone)]
pub struct PostgresCheckpointStore {
    pool: PgPool,
}

impl PostgresCheckpointStore {
    /// Creates a checkpoint store backed by the given pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Returns the pool used by this store.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

impl CheckpointStore for PostgresCheckpointStore {
    type Error = CheckpointError;

    async fn load(&self, view: &str) -> Result<Option<SequencePosition>, Self::Error> {
        let position: Option<i64> = sqlx::query_scalar(
            "SELECT last_event_position FROM projection_checkpoints WHERE view_name = $1",
        )
        .bind(view)
        .fetch_optional(&self.pool)
        .await?;

        match position {
            Some(position) => u64::try_from(position)
                .map(Some)
                .map_err(|_| CheckpointError::PositionOverflow),
            None => Ok(None),
        }
    }

    async fn save(&self, view: &str, position: SequencePosition) -> Result<(), Self::Error> {
        let position = i64::try_from(position).map_err(|_| CheckpointError::PositionOverflow)?;

        sqlx::query(
            r#"
            INSERT INTO projection_checkpoints (view_name, last_event_position, updated_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (view_name) DO UPDATE SET
                last_event_position = GREATEST(
                    projection_checkpoints.last_event_position,
                    EXCLUDED.last_event_position
                ),
                updated_at = NOW()
            "#,
        )
        .bind(view)
        .bind(position)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use sqlx::postgres::PgPoolOptions;

    use super::*;

    #[tokio::test]
    async fn rejects_positions_outside_bigint() -> Result<(), Box<dyn std::error::Error>> {
        let pool =
            PgPoolOptions::new().connect_lazy("postgres://postgres:password@localhost:5432/dev")?;
        let store = PostgresCheckpointStore::new(pool);

        let result = store.save("checkpoint:test", u64::MAX).await;

        assert!(matches!(result, Err(CheckpointError::PositionOverflow)));
        Ok(())
    }
}
