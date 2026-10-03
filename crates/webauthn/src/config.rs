use serde::{Deserialize, Serialize};
use url::Url;

/// Configuration for WebAuthn Relying Party.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebauthnConfig {
    /// Relying Party ID (domain name, e.g. "localhost" or "example.com").
    pub rp_id: String,
    /// Relying Party Origin (scheme + domain + optional port, e.g. "http://localhost:8080").
    pub rp_origin: Url,
    /// Relying Party human-readable name (e.g. "Backend App").
    pub rp_name: String,
}

impl Default for WebauthnConfig {
    fn default() -> Self {
        #[allow(clippy::expect_used)]
        let rp_origin =
            Url::parse("http://localhost:8080").expect("default WebAuthn origin must be valid URL");
        Self {
            rp_id: "localhost".to_string(),
            rp_origin,
            rp_name: "Backend App".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = WebauthnConfig::default();
        assert_eq!(config.rp_id, "localhost");
        assert_eq!(config.rp_name, "Backend App");
        assert_eq!(config.rp_origin.as_str(), "http://localhost:8080/");
    }

    #[test]
    fn deserializes_config() {
        let json =
            r#"{"rp_id":"example.com","rp_origin":"https://example.com","rp_name":"My App"}"#;
        #[allow(clippy::unwrap_used)]
        let config: WebauthnConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.rp_id, "example.com");
        assert_eq!(config.rp_name, "My App");
        assert_eq!(config.rp_origin.as_str(), "https://example.com/");
    }
}
