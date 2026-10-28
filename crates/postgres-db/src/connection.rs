use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

use crate::metrics::spawn_pool_metrics_monitor;
use crate::{PostgresConfig, PostgresSecret};

/// Builds PostgreSQL connection options from config and credentials.
pub fn connect_options(config: &PostgresConfig, secret: &PostgresSecret) -> PgConnectOptions {
    PgConnectOptions::new()
        .host(&config.host)
        .port(config.port)
        .database(&config.database)
        .username(&secret.username)
        .password(&secret.password)
        .ssl_mode(config.ssl_mode.into())
}

/// Initializes a PostgreSQL connection pool from config and credentials.
pub async fn init_pool(
    config: &PostgresConfig,
    secret: &PostgresSecret,
) -> Result<PgPool, sqlx::Error> {
    let options = connect_options(config, secret);
    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(Duration::from_secs(config.connect_timeout_secs))
        .idle_timeout(Duration::from_secs(config.idle_timeout_secs))
        .max_lifetime(Duration::from_secs(config.max_lifetime_secs))
        .connect_with(options)
        .await?;

    spawn_pool_metrics_monitor(pool.clone(), config.max_connections, Duration::from_secs(5));

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_connect_options_from_defaults() {
        let config = PostgresConfig::default();
        let secret = PostgresSecret::default();
        let options = connect_options(&config, &secret);
        assert_eq!(options.get_host(), "localhost");
        assert_eq!(options.get_port(), 5432);
        assert_eq!(options.get_database(), Some("dev"));
        assert_eq!(options.get_username(), "postgres");
        assert!(matches!(
            options.get_ssl_mode(),
            sqlx::postgres::PgSslMode::Prefer
        ));
    }
}
