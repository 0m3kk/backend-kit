//! Projection checkpoints, view contract, catch-up worker, and runner.

pub mod checkpoint;
pub mod runner;
pub mod view;
pub mod worker;

pub use checkpoint::{
    CheckpointError, CheckpointStore, PROJECTION_MIGRATOR, PostgresCheckpointStore, migrator,
};
pub use runner::{CatchupConfig, DEFAULT_CATCHUP_INTERVAL, run_catchup_loop};
pub use view::View;
pub use worker::{CatchupReport, ProjectionWorker, WorkerError};
