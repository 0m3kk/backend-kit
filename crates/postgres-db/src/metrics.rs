//! PostgreSQL connection pool metrics recording and background monitor.

use std::time::Duration;

use sqlx::PgPool;
use telemetry::ValueGauge;
use tokio::time::interval;

/// Connection pool metrics holder.
#[derive(Clone)]
pub struct PoolMetrics {
    connections_size: ValueGauge,
    connections_idle: ValueGauge,
    connections_in_use: ValueGauge,
    connections_max: ValueGauge,
}

impl Default for PoolMetrics {
    fn default() -> Self {
        Self {
            connections_size: ValueGauge::with_scope("postgres-db", "db_pool_connections_size"),
            connections_idle: ValueGauge::with_scope("postgres-db", "db_pool_connections_idle"),
            connections_in_use: ValueGauge::with_scope("postgres-db", "db_pool_connections_in_use"),
            connections_max: ValueGauge::with_scope("postgres-db", "db_pool_connections_max"),
        }
    }
}

impl PoolMetrics {
    /// Records current snapshot of pool connection statistics.
    pub fn record_pool_stats(&self, pool: &PgPool, max_connections: u32) {
        let size = pool.size() as u64;
        let idle = pool.num_idle() as u64;
        let in_use = size.saturating_sub(idle);

        self.connections_size.record(size);
        self.connections_idle.record(idle);
        self.connections_in_use.record(in_use);
        self.connections_max.record(max_connections as u64);
    }
}

/// Spawns a background task that periodically samples pool metrics.
pub fn spawn_pool_metrics_monitor(
    pool: PgPool,
    max_connections: u32,
    sample_interval: Duration,
) -> tokio::task::JoinHandle<()> {
    let metrics = PoolMetrics::default();
    tokio::spawn(async move {
        let mut ticker = interval(sample_interval);
        loop {
            ticker.tick().await;
            if pool.is_closed() {
                break;
            }
            metrics.record_pool_stats(&pool, max_connections);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_metrics_creates_valid_instance() {
        let _metrics = PoolMetrics::default();
    }
}
