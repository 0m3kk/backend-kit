use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::cookie::CookieConfig;
use crate::cors::CorsConfig;
use crate::security_headers::SecurityHeadersConfig;

/// HTTP server configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpConfig {
    /// Host address to bind the HTTP server to.
    #[serde(default = "default_host")]
    pub host: String,

    /// Port to listen on.
    #[serde(default = "default_port")]
    pub port: u16,

    /// Maximum request body size in bytes.
    #[serde(default = "default_body_limit_bytes")]
    pub body_limit_bytes: usize,

    /// Request timeout in seconds. None disables timeout.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: Option<u64>,

    /// Whether to enable response compression (gzip, brotli, zstd).
    #[serde(default = "default_compression")]
    pub compression: bool,

    /// Optional security headers configuration. If `None`, security headers are omitted.
    #[serde(default = "default_security_headers")]
    pub security_headers: Option<SecurityHeadersConfig>,

    /// CORS configuration.
    #[serde(default)]
    pub cors: CorsConfig,

    /// Cookie configuration.
    #[serde(default)]
    pub cookie: CookieConfig,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

const fn default_port() -> u16 {
    8080
}

const fn default_body_limit_bytes() -> usize {
    2 * 1024 * 1024
}

const fn default_timeout_secs() -> Option<u64> {
    Some(30)
}

const fn default_compression() -> bool {
    true
}

fn default_security_headers() -> Option<SecurityHeadersConfig> {
    Some(SecurityHeadersConfig::default())
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            body_limit_bytes: default_body_limit_bytes(),
            timeout_secs: default_timeout_secs(),
            compression: default_compression(),
            security_headers: default_security_headers(),
            cors: CorsConfig::default(),
            cookie: CookieConfig::default(),
        }
    }
}

impl HttpConfig {
    /// Returns the socket address to bind to.
    pub fn socket_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.host, self.port).parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let config = HttpConfig::default();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert_eq!(config.body_limit_bytes, 2 * 1024 * 1024);
        assert_eq!(config.timeout_secs, Some(30));
        assert!(config.compression);
        assert_eq!(
            config.security_headers,
            Some(SecurityHeadersConfig::default())
        );
        assert_eq!(config.cors, CorsConfig::default());
        assert_eq!(config.cookie, CookieConfig::default());
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        #[allow(clippy::unwrap_used)]
        let config: HttpConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config, HttpConfig::default());
    }

    #[test]
    fn deserialize_custom_values() {
        let json = r#"{
            "host": "127.0.0.1",
            "port": 3000,
            "body_limit_bytes": 1048576,
            "timeout_secs": 15,
            "compression": false,
            "security_headers": {
                "frame_options": "SAMEORIGIN",
                "xss_protection": null
            },
            "cors": {
                "allow_origins": ["http://localhost:3000"]
            },
            "cookie": {
                "secure": true,
                "same_site": "strict"
            }
        }"#;
        #[allow(clippy::unwrap_used)]
        let config: HttpConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 3000);
        assert_eq!(config.body_limit_bytes, 1048576);
        assert_eq!(config.timeout_secs, Some(15));
        assert!(!config.compression);

        #[allow(clippy::unwrap_used)]
        let sec = config.security_headers.unwrap();
        assert_eq!(sec.frame_options, Some("SAMEORIGIN".to_string()));
        assert_eq!(sec.xss_protection, None);
        assert_eq!(sec.content_type_options, Some("nosniff".to_string()));

        assert_eq!(config.cors.allow_origins, vec!["http://localhost:3000"]);
        assert!(config.cookie.secure);
        assert_eq!(config.cookie.same_site, crate::cookie::SameSite::Strict);
    }

    #[test]
    fn parses_valid_socket_addr() {
        let config = HttpConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            ..Default::default()
        };
        #[allow(clippy::unwrap_used)]
        let addr = config.socket_addr().unwrap();
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_eq!(addr.port(), 8080);
    }
}
