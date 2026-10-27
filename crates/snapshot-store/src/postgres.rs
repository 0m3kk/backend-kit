use async_trait::async_trait;
use sqlx::{PgPool, Row};

use crate::{Snapshot, SnapshotStore};

/// Embedded database migrations for snapshots.
///
/// This uses the default `_sqlx_migrations` table, so applications that own
/// migrations of their own should prefer [`migrator`].
pub static SNAPSHOT_STORE_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Returns the snapshot store migrations with a dedicated migration table.
///
/// The store keeps its bookkeeping in `_snapshot_store_migrations`, so running
/// these migrations cannot conflict with an application's own migrations in
/// `_sqlx_migrations`.
pub fn migrator() -> sqlx::migrate::Migrator {
    let mut migrator = sqlx::migrate!("./migrations");
    migrator.dangerous_set_table_name("_snapshot_store_migrations");
    migrator
}

/// A PostgreSQL-backed snapshot store.
#[derive(Debug, Clone)]
pub struct PostgresSnapshotStore {
    pool: PgPool,
    threshold: Option<u64>,
}

impl PostgresSnapshotStore {
    /// Create a new PostgreSQL snapshot store with the given connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            threshold: None,
        }
    }

    /// Configure a minimum event threshold before saving snapshots.
    pub fn with_threshold(mut self, threshold: u64) -> Self {
        self.threshold = Some(threshold);
        self
    }

    /// Returns the configured threshold, if any.
    pub fn threshold(&self) -> Option<u64> {
        self.threshold
    }
}

#[async_trait]
impl SnapshotStore for PostgresSnapshotStore {
    type Error = sqlx::Error;

    async fn get(&self, key: &str) -> Result<Option<Snapshot>, Self::Error> {
        let row = sqlx::query("SELECT position, state FROM snapshots WHERE key = $1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| {
            let position = r.get::<Option<i64>, _>(0).map(|p| p as u64);
            let state = r.get(1);
            Snapshot { position, state }
        }))
    }

    async fn put(&self, key: &str, snapshot: Snapshot) -> Result<(), Self::Error> {
        let position = snapshot.position.map(|p| p as i64);
        sqlx::query(
            "INSERT INTO snapshots (key, position, state, updated_at) \
             VALUES ($1, $2, $3, NOW()) \
             ON CONFLICT (key) DO UPDATE SET \
               position = EXCLUDED.position, \
               state = EXCLUDED.state, \
               updated_at = NOW()",
        )
        .bind(key)
        .bind(position)
        .bind(snapshot.state)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    fn should_snapshot(&self, replayed_events: u64) -> bool {
        match self.threshold {
            Some(threshold) => replayed_events >= threshold,
            None => true,
        }
    }
}
