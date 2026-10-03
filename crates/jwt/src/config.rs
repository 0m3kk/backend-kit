use serde::{Deserialize, Serialize};

/// Configuration parameters for JWT tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JwtConfig {
    /// Token lifetime in seconds (default: 86400 / 24 hours).
    #[serde(default = "default_expiration_secs")]
    pub expiration_secs: u64,
}

fn default_expiration_secs() -> u64 {
    86400
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            expiration_secs: default_expiration_secs(),
        }
    }
}

/// Secret key for HMAC signing and verifying JWT tokens.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JwtSecret {
    /// Signing secret key.
    pub secret: String,
}

impl std::fmt::Debug for JwtSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtSecret")
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

impl Default for JwtSecret {
    fn default() -> Self {
        Self {
            secret: "change_me_super_secret_jwt_key_at_least_32_bytes_long".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_jwt_secret() {
        let secret = JwtSecret {
            secret: "super_secret_jwt_key_12345".to_string(),
        };
        let debug_str = format!("{secret:?}");
        assert!(!debug_str.contains("super_secret_jwt_key_12345"));
        assert!(debug_str.contains("[REDACTED]"));
    }
}
