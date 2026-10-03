use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::postgres::PgConnectOptions;

use crate::PostgresSecret;
use crate::connection;

/// PostgreSQL connection and pool configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostgresConfig {
    /// Database server hostname or IP address.
    #[serde(default = "default_host")]
    pub host: String,

    /// Database server port.
    #[serde(default = "default_port")]
    pub port: u16,

    /// Name of the target database.
    #[serde(default = "default_database")]
    pub database: String,

    /// Maximum number of connections in the pool.
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Minimum number of idle connections maintained by the pool.
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    /// Connection acquisition timeout in seconds.
    #[serde(default = "default_connect_timeout_secs")]
    pub connect_timeout_secs: u64,

    /// Maximum idle duration in seconds before closing a connection.
    #[serde(default = "default_idle_timeout_secs")]
    pub idle_timeout_secs: u64,

    /// Maximum lifetime of a connection in seconds.
    #[serde(default = "default_max_lifetime_secs")]
    pub max_lifetime_secs: u64,
}

impl PostgresConfig {
    /// Builds PostgreSQL connection options using provided credentials.
    pub fn connect_options(&self, secret: &PostgresSecret) -> PgConnectOptions {
        connection::connect_options(self, secret)
    }

    /// Initializes a PostgreSQL connection pool using provided credentials.
    pub async fn init_pool(&self, secret: &PostgresSecret) -> Result<PgPool, sqlx::Error> {
        connection::init_pool(self, secret).await
    }
}

fn default_host() -> String {
    "localhost".to_string()
}

const fn default_port() -> u16 {
    5432
}

fn default_database() -> String {
    "dev".to_string()
}

const fn default_max_connections() -> u32 {
    10
}

const fn default_min_connections() -> u32 {
    1
}

const fn default_connect_timeout_secs() -> u64 {
    30
}

const fn default_idle_timeout_secs() -> u64 {
    600
}

const fn default_max_lifetime_secs() -> u64 {
    1800
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            database: default_database(),
            max_connections: default_max_connections(),
            min_connections: default_min_connections(),
            connect_timeout_secs: default_connect_timeout_secs(),
            idle_timeout_secs: default_idle_timeout_secs(),
            max_lifetime_secs: default_max_lifetime_secs(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let config = PostgresConfig::default();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 5432);
        assert_eq!(config.database, "dev");
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.min_connections, 1);
        assert_eq!(config.connect_timeout_secs, 30);
        assert_eq!(config.idle_timeout_secs, 600);
        assert_eq!(config.max_lifetime_secs, 1800);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        #[allow(clippy::unwrap_used)]
        let config: PostgresConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config, PostgresConfig::default());
    }

    #[test]
    fn deserialize_custom_values() {
        let json = r#"{
            "host": "db.example.com",
            "port": 5433,
            "database": "production",
            "max_connections": 50,
            "min_connections": 5,
            "connect_timeout_secs": 10,
            "idle_timeout_secs": 300,
            "max_lifetime_secs": 900
        }"#;
        #[allow(clippy::unwrap_used)]
        let config: PostgresConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.host, "db.example.com");
        assert_eq!(config.port, 5433);
        assert_eq!(config.database, "production");
        assert_eq!(config.max_connections, 50);
        assert_eq!(config.min_connections, 5);
        assert_eq!(config.connect_timeout_secs, 10);
        assert_eq!(config.idle_timeout_secs, 300);
        assert_eq!(config.max_lifetime_secs, 900);
    }
}
