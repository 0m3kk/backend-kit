use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// OpenTelemetry secret credentials and authorization headers.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TelemetrySecret {
    /// Authorization header value (e.g. `Basic YWRtaW5AZXhhbXBsZS5jb206Q29tcGxleFBhc3N3b3JkMTIzIQ==` or `Bearer <token>`).
    #[serde(default)]
    pub authorization: Option<String>,

    /// Additional custom OTLP export headers.
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

impl std::fmt::Debug for TelemetrySecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sanitized_headers: HashMap<&String, &str> =
            self.headers.keys().map(|k| (k, "[REDACTED]")).collect();
        f.debug_struct("TelemetrySecret")
            .field(
                "authorization",
                &self.authorization.as_ref().map(|_| "[REDACTED]"),
            )
            .field("headers", &sanitized_headers)
            .finish()
    }
}

impl TelemetrySecret {
    /// Combines authorization and custom headers into a single map.
    pub fn to_headers(&self) -> HashMap<String, String> {
        let mut map = self.headers.clone();
        if let Some(ref auth) = self.authorization {
            let trimmed = auth.trim();
            if !trimmed.is_empty() {
                map.insert("authorization".to_string(), trimmed.to_string());
                map.entry("organization".to_string())
                    .or_insert_with(|| "default".to_string());
            }
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_secret_is_empty() {
        let secret = TelemetrySecret::default();
        assert!(secret.authorization.is_none());
        assert!(secret.headers.is_empty());
        assert!(secret.to_headers().is_empty());
    }

    #[test]
    fn to_headers_combines_authorization_and_headers() {
        let mut custom = HashMap::new();
        custom.insert("x-scope-orgid".to_string(), "default".to_string());

        let secret = TelemetrySecret {
            authorization: Some("Basic dXNlcjpwYXNz".to_string()),
            headers: custom,
        };

        let headers = secret.to_headers();
        assert_eq!(
            headers.get("authorization").map(String::as_str),
            Some("Basic dXNlcjpwYXNz")
        );
        assert_eq!(
            headers.get("x-scope-orgid").map(String::as_str),
            Some("default")
        );
        assert_eq!(
            headers.get("organization").map(String::as_str),
            Some("default")
        );
    }

    #[test]
    fn debug_redacts_telemetry_secret() {
        let mut custom = HashMap::new();
        custom.insert("api-key".to_string(), "secret_api_key_123".to_string());
        let secret = TelemetrySecret {
            authorization: Some("Bearer super_secret_token".to_string()),
            headers: custom,
        };
        let debug_str = format!("{secret:?}");
        assert!(!debug_str.contains("super_secret_token"));
        assert!(!debug_str.contains("secret_api_key_123"));
        assert!(debug_str.contains("[REDACTED]"));
    }
}
