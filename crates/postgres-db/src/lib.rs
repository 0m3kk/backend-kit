//! PostgreSQL configuration, secrets, and connection management.

mod config;
mod connection;
pub mod metrics;
mod secret;

pub use config::PostgresConfig;
pub use connection::{connect_options, init_pool};
pub use metrics::{PoolMetrics, spawn_pool_metrics_monitor};
pub use secret::PostgresSecret;

/// Environment variable holding a PostgreSQL connection URL.
pub const ENV_DATABASE_URL: &str = "DATABASE_URL";
