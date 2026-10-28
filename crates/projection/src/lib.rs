//! Projection checkpoints, view contract, catch-up supervisor, and leader election.

pub mod checkpoint;
pub mod health;
pub mod leader;
mod metrics;
pub mod notifications;
pub mod projection;
pub mod shutdown;
pub mod supervisor;

pub use checkpoint::{
    CheckpointError, CheckpointStore, PROJECTION_MIGRATOR, PostgresCheckpointStore, migrator,
    save_in_transaction,
};
pub use health::{ProjectionStatus, UNHEALTHY_AFTER_ERRORS, WorkerHealth};
pub use leader::{LeaderError, LeaderGuard};
pub use notifications::spawn_listener;
pub use projection::{Projection, ProjectionError, ProjectionFuture};
pub use shutdown::shutdown_signal;
pub use supervisor::{
    CatchupConfig, CatchupReport, DEFAULT_CATCHUP_DEBOUNCE, DEFAULT_CATCHUP_INTERVAL,
    ProjectionSupervisor, SupervisorError, WorkerConfig,
};
