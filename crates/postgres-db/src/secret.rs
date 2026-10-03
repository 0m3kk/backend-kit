use serde::{Deserialize, Serialize};

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
}
