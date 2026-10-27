//! DCB compliant snapshot store: types, [`SnapshotStore`] trait, and PostgreSQL implementation.

mod postgres;
mod snapshot;
mod store;

pub use postgres::{PostgresSnapshotStore, SNAPSHOT_STORE_MIGRATOR, migrator};
pub use snapshot::Snapshot;
pub use store::{MockSnapshotStore, SnapshotStore};
