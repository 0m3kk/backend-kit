//! CORS configuration and layer builder.

use std::time::Duration;

use axum::http::{HeaderName, HeaderValue, Method};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

/// CORS configuration parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorsConfig {
    /// Allowed origins. Use `["*"]` for wildcard.
    #[serde(default = "default_cors_origins")]
    pub allow_origins: Vec<String>,

    /// Allowed HTTP methods. Use `["*"]` for any method.
    #[serde(default = "default_cors_methods")]
    pub allow_methods: Vec<String>,

    /// Allowed headers. Use `["*"]` for any header.
    #[serde(default = "default_cors_headers")]
    pub allow_headers: Vec<String>,

    /// Exposed response headers.
    #[serde(default)]
    pub expose_headers: Vec<String>,

    /// Whether requests can include user credentials.
    #[serde(default)]
    pub allow_credentials: bool,

    /// Maximum age in seconds for CORS preflight cache.
    #[serde(default)]
    pub max_age_secs: Option<u64>,
}

fn default_cors_origins() -> Vec<String> {
    vec!["*".to_string()]
}

fn default_cors_methods() -> Vec<String> {
    vec!["*".to_string()]
}

fn default_cors_headers() -> Vec<String> {
    vec!["*".to_string()]
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allow_origins: default_cors_origins(),
            allow_methods: default_cors_methods(),
            allow_headers: default_cors_headers(),
            expose_headers: Vec::new(),
            allow_credentials: false,
            max_age_secs: None,
        }
    }
}

impl CorsConfig {
    /// Builds a [`CorsLayer`] from the configuration.
    pub fn build_layer(&self) -> CorsLayer {
        let mut layer = CorsLayer::new();

        if self.allow_origins.iter().any(|o| o == "*") {
            layer = layer.allow_origin(Any);
        } else {
            let origins: Vec<HeaderValue> = self
                .allow_origins
                .iter()
                .filter_map(|o| HeaderValue::from_str(o).ok())
                .collect();
            layer = layer.allow_origin(origins);
        }

        if self.allow_methods.iter().any(|m| m == "*") {
            layer = layer.allow_methods(Any);
        } else {
            let methods: Vec<Method> = self
                .allow_methods
                .iter()
                .filter_map(|m| Method::from_bytes(m.as_bytes()).ok())
                .collect();
            layer = layer.allow_methods(methods);
        }

        if self.allow_headers.iter().any(|h| h == "*") {
            layer = layer.allow_headers(Any);
        } else {
            let headers: Vec<HeaderName> = self
                .allow_headers
                .iter()
                .filter_map(|h| HeaderName::from_bytes(h.as_bytes()).ok())
                .collect();
            layer = layer.allow_headers(headers);
        }

        if !self.expose_headers.is_empty() {
            let expose: Vec<HeaderName> = self
                .expose_headers
                .iter()
                .filter_map(|h| HeaderName::from_bytes(h.as_bytes()).ok())
                .collect();
            layer = layer.expose_headers(expose);
        }

        if self.allow_credentials {
            layer = layer.allow_credentials(true);
        }

        if let Some(max_age) = self.max_age_secs {
            layer = layer.max_age(Duration::from_secs(max_age));
        }

        layer
    }
}

/// Builds a [`CorsLayer`] from the provided configuration.
pub fn cors_layer(config: &CorsConfig) -> CorsLayer {
    config.build_layer()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cors_config() {
        let config = CorsConfig::default();
        assert_eq!(config.allow_origins, vec!["*"]);
        assert_eq!(config.allow_methods, vec!["*"]);
        assert_eq!(config.allow_headers, vec!["*"]);
        assert!(!config.allow_credentials);
        assert_eq!(config.max_age_secs, None);
    }

    #[test]
    fn builds_cors_layer_with_wildcard() {
        let config = CorsConfig::default();
        let _layer = config.build_layer();
    }

    #[test]
    fn builds_cors_layer_with_specific_configuration() {
        let config = CorsConfig {
            allow_origins: vec!["https://example.com".to_string()],
            allow_methods: vec!["GET".to_string(), "POST".to_string()],
            allow_headers: vec!["Authorization".to_string(), "Content-Type".to_string()],
            expose_headers: vec!["X-Custom-Header".to_string()],
            allow_credentials: true,
            max_age_secs: Some(3600),
        };
        let _layer = config.build_layer();
    }
}
