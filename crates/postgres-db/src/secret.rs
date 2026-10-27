use serde::{Deserialize, Serialize};

use crate::ENV_DATABASE_URL;

/// PostgreSQL authentication credentials.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostgresSecret {
    /// Username for authentication.
    #[serde(default = "default_username")]
    pub username: String,

    /// Password for authentication.
    #[serde(default = "default_password")]
    pub password: String,
}

impl std::fmt::Debug for PostgresSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresSecret")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

fn default_username() -> String {
    "postgres".to_string()
}

fn default_password() -> String {
    "password".to_string()
}

impl Default for PostgresSecret {
    fn default() -> Self {
        Self {
            username: default_username(),
            password: default_password(),
        }
    }
}

impl PostgresSecret {
    /// Applies the username and password from the `DATABASE_URL` environment
    /// variable, when it is set.
    pub fn apply_database_url_env(&mut self) -> Result<(), url::ParseError> {
        match std::env::var(ENV_DATABASE_URL) {
            Ok(url) => self.apply_database_url(&url),
            Err(_) => Ok(()),
        }
    }

    /// Applies the username and password from a connection URL.
    pub fn apply_database_url(&mut self, url: &str) -> Result<(), url::ParseError> {
        let parsed = url::Url::parse(url)?;

        let username = parsed.username();
        if !username.is_empty() {
            self.username = username.to_string();
        }
        if let Some(password) = parsed.password() {
            self.password = password.to_string();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_secret_has_expected_values() {
        let secret = PostgresSecret::default();
        assert_eq!(secret.username, "postgres");
        assert_eq!(secret.password, "password");
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        #[allow(clippy::unwrap_used)]
        let secret: PostgresSecret = serde_json::from_str("{}").unwrap();
        assert_eq!(secret, PostgresSecret::default());
    }

    #[test]
    fn deserialize_custom_secret() {
        let json = r#"{
            "username": "custom_user",
            "password": "super_secret_password"
        }"#;
        #[allow(clippy::unwrap_used)]
        let secret: PostgresSecret = serde_json::from_str(json).unwrap();
        assert_eq!(secret.username, "custom_user");
        assert_eq!(secret.password, "super_secret_password");
    }

    #[test]
    fn debug_redacts_password() {
        let secret = PostgresSecret {
            username: "admin".to_string(),
            password: "super_secret_password".to_string(),
        };
        let debug_str = format!("{secret:?}");
        assert!(!debug_str.contains("super_secret_password"));
        assert!(debug_str.contains("[REDACTED]"));
    }

    #[test]
    fn applies_database_url() -> Result<(), Box<dyn std::error::Error>> {
        let mut secret = PostgresSecret::default();

        secret.apply_database_url("postgres://catalog:secret@db.example.com:5432/dev")?;

        assert_eq!(secret.username, "catalog");
        assert_eq!(secret.password, "secret");
        Ok(())
    }

    #[test]
    fn rejects_malformed_database_url() {
        let mut secret = PostgresSecret::default();
        assert!(secret.apply_database_url("not a url").is_err());
    }
}
