//! Projection checkpoints, view contract, catch-up worker, and runner.

pub mod checkpoint;
pub mod health;
pub mod leader;
pub mod notifications;
pub mod projection;
pub mod runner;
pub mod shutdown;
pub mod supervisor;
pub mod view;
pub mod worker;

pub use checkpoint::{
    CheckpointError, CheckpointStore, PROJECTION_MIGRATOR, PostgresCheckpointStore, migrator,
};
pub use health::{ProjectionStatus, WorkerHealth};
pub use leader::{LeaderError, LeaderGuard};
pub use notifications::spawn_listener;
pub use projection::{Projection, ProjectionError, ProjectionFuture};
pub use runner::{
    CatchupConfig, DEFAULT_CATCHUP_DEBOUNCE, DEFAULT_CATCHUP_INTERVAL, run_catchup_loop,
    run_catchup_loop_with_wake,
};
pub use shutdown::shutdown_signal;
pub use supervisor::{ProjectionSupervisor, SupervisorError, WorkerConfig};
pub use view::View;
pub use worker::{CatchupReport, ProjectionWorker, WorkerError};
