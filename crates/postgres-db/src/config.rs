use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgSslMode};

use crate::ENV_DATABASE_URL;
use crate::PostgresSecret;
use crate::connection;

/// TLS mode used for PostgreSQL connections.
///
/// Mirrors PostgreSQL's `sslmode` values and is applied when a
/// `DATABASE_URL` carries `?sslmode=...`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SslMode {
    /// Never use TLS.
    Disable,
    /// Try TLS first, then fall back to plaintext.
    Allow,
    /// Try TLS first, then fall back to plaintext (driver default).
    #[default]
    Prefer,
    /// Require TLS but do not verify the server certificate.
    Require,
    /// Require TLS and verify the certificate authority.
    VerifyCa,
    /// Require TLS and verify the certificate authority and hostname.
    VerifyFull,
}

impl SslMode {
    /// Stable string representation matching PostgreSQL's `sslmode` values.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disable => "disable",
            Self::Allow => "allow",
            Self::Prefer => "prefer",
            Self::Require => "require",
            Self::VerifyCa => "verify-ca",
            Self::VerifyFull => "verify-full",
        }
    }
}

impl FromStr for SslMode {
    type Err = ConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "disable" => Ok(Self::Disable),
            "allow" => Ok(Self::Allow),
            "prefer" => Ok(Self::Prefer),
            "require" => Ok(Self::Require),
            "verify-ca" => Ok(Self::VerifyCa),
            "verify-full" => Ok(Self::VerifyFull),
            other => Err(ConfigError::InvalidSslMode(other.to_string())),
        }
    }
}

impl From<SslMode> for PgSslMode {
    fn from(value: SslMode) -> Self {
        match value {
            SslMode::Disable => Self::Disable,
            SslMode::Allow => Self::Allow,
            SslMode::Prefer => Self::Prefer,
            SslMode::Require => Self::Require,
            SslMode::VerifyCa => Self::VerifyCa,
            SslMode::VerifyFull => Self::VerifyFull,
        }
    }
}

/// Errors produced while applying database configuration values.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The database URL could not be parsed.
    #[error("invalid database URL: {0}")]
    Url(#[from] url::ParseError),

    /// An unknown `sslmode` value was provided.
    #[error(
        "invalid sslmode `{0}`; expected disable, allow, prefer, require, verify-ca, or verify-full"
    )]
    InvalidSslMode(String),
}

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

    /// TLS mode used for the connection.
    #[serde(default)]
    pub ssl_mode: SslMode,
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

    /// Applies host, port, database, and `sslmode` from the `DATABASE_URL`
    /// environment variable, when it is set.
    pub fn apply_database_url_env(&mut self) -> Result<(), ConfigError> {
        match std::env::var(ENV_DATABASE_URL) {
            Ok(url) => self.apply_database_url(&url),
            Err(_) => Ok(()),
        }
    }

    /// Applies host, port, database, and `sslmode` from a connection URL.
    pub fn apply_database_url(&mut self, url: &str) -> Result<(), ConfigError> {
        let parsed = url::Url::parse(url)?;

        if let Some(host) = parsed.host_str() {
            self.host = host.to_string();
        }
        if let Some(port) = parsed.port() {
            self.port = port;
        }
        let database = parsed.path().trim_start_matches('/');
        if !database.is_empty() {
            self.database = database.to_string();
        }

        for (key, value) in parsed.query_pairs() {
            if key.eq_ignore_ascii_case("sslmode") {
                self.ssl_mode = value.parse()?;
            }
        }

        Ok(())
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
            ssl_mode: SslMode::default(),
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
        assert_eq!(config.ssl_mode, SslMode::Prefer);
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
            "max_lifetime_secs": 900,
            "ssl_mode": "verify-full"
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
        assert_eq!(config.ssl_mode, SslMode::VerifyFull);
    }

    #[test]
    fn applies_database_url() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = PostgresConfig::default();

        config.apply_database_url(
            "postgres://postgres:password@db.example.com:6543/catalog?sslmode=require",
        )?;

        assert_eq!(config.host, "db.example.com");
        assert_eq!(config.port, 6543);
        assert_eq!(config.database, "catalog");
        assert_eq!(config.ssl_mode, SslMode::Require);
        Ok(())
    }

    #[test]
    fn rejects_malformed_database_url() {
        let mut config = PostgresConfig::default();
        assert!(matches!(
            config.apply_database_url("not a url"),
            Err(ConfigError::Url(_))
        ));
    }

    #[test]
    fn rejects_unknown_sslmode() {
        let mut config = PostgresConfig::default();
        assert!(matches!(
            config.apply_database_url("postgres://localhost/dev?sslmode=insecure"),
            Err(ConfigError::InvalidSslMode(_))
        ));
    }
}
