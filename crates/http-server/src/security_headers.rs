//! Security headers configuration and middleware builder.

use axum::Router;
use axum::http::header::{HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use tower_http::set_header::SetResponseHeaderLayer;

/// Header name for `X-Content-Type-Options`.
pub static X_CONTENT_TYPE_OPTIONS: HeaderName = HeaderName::from_static("x-content-type-options");

/// Header name for `X-Frame-Options`.
pub static X_FRAME_OPTIONS: HeaderName = HeaderName::from_static("x-frame-options");

/// Header name for `X-XSS-Protection`.
pub static X_XSS_PROTECTION: HeaderName = HeaderName::from_static("x-xss-protection");

/// Header name for `Strict-Transport-Security`.
pub static STRICT_TRANSPORT_SECURITY: HeaderName =
    HeaderName::from_static("strict-transport-security");

/// Header name for `Content-Security-Policy`.
pub static CONTENT_SECURITY_POLICY: HeaderName = HeaderName::from_static("content-security-policy");

/// Header name for `Referrer-Policy`.
pub static REFERRER_POLICY: HeaderName = HeaderName::from_static("referrer-policy");

/// Configurable security headers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityHeadersConfig {
    /// `X-Content-Type-Options` header value (e.g. `"nosniff"`).
    #[serde(default = "default_content_type_options")]
    pub content_type_options: Option<String>,

    /// `X-Frame-Options` header value (e.g. `"DENY"`, `"SAMEORIGIN"`).
    #[serde(default = "default_frame_options")]
    pub frame_options: Option<String>,

    /// `X-XSS-Protection` header value (e.g. `"0"`).
    #[serde(default = "default_xss_protection")]
    pub xss_protection: Option<String>,

    /// `Strict-Transport-Security` (HSTS) header value.
    #[serde(default = "default_hsts")]
    pub hsts: Option<String>,

    /// `Content-Security-Policy` header value.
    #[serde(default)]
    pub content_security_policy: Option<String>,

    /// `Referrer-Policy` header value.
    #[serde(default = "default_referrer_policy")]
    pub referrer_policy: Option<String>,
}

fn default_content_type_options() -> Option<String> {
    Some("nosniff".to_string())
}

fn default_frame_options() -> Option<String> {
    Some("DENY".to_string())
}

fn default_xss_protection() -> Option<String> {
    Some("0".to_string())
}

fn default_hsts() -> Option<String> {
    Some("max-age=31536000; includeSubDomains".to_string())
}

fn default_referrer_policy() -> Option<String> {
    Some("strict-origin-when-cross-origin".to_string())
}

impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self {
            content_type_options: default_content_type_options(),
            frame_options: default_frame_options(),
            xss_protection: default_xss_protection(),
            hsts: default_hsts(),
            content_security_policy: None,
            referrer_policy: default_referrer_policy(),
        }
    }
}

/// Parses one configured header value and logs a warning instead of silently
/// dropping an invalid value.
fn parse_header(name: &str, value: &str) -> Option<HeaderValue> {
    match HeaderValue::from_str(value) {
        Ok(value) => Some(value),
        Err(error) => {
            tracing::warn!(
                header = name,
                %error,
                "invalid security header value; header disabled"
            );
            None
        }
    }
}

impl SecurityHeadersConfig {
    /// Applies the configured security headers to the router.
    pub fn apply(&self, mut router: Router) -> Router {
        if let Some(val) = self
            .content_type_options
            .as_deref()
            .and_then(|v| parse_header("x-content-type-options", v))
        {
            router = router.layer(SetResponseHeaderLayer::overriding(
                X_CONTENT_TYPE_OPTIONS.clone(),
                val,
            ));
        }

        if let Some(val) = self
            .frame_options
            .as_deref()
            .and_then(|v| parse_header("x-frame-options", v))
        {
            router = router.layer(SetResponseHeaderLayer::overriding(
                X_FRAME_OPTIONS.clone(),
                val,
            ));
        }

        if let Some(val) = self
            .xss_protection
            .as_deref()
            .and_then(|v| parse_header("x-xss-protection", v))
        {
            router = router.layer(SetResponseHeaderLayer::overriding(
                X_XSS_PROTECTION.clone(),
                val,
            ));
        }

        if let Some(val) = self
            .hsts
            .as_deref()
            .and_then(|v| parse_header("strict-transport-security", v))
        {
            router = router.layer(SetResponseHeaderLayer::if_not_present(
                STRICT_TRANSPORT_SECURITY.clone(),
                val,
            ));
        }

        if let Some(val) = self
            .content_security_policy
            .as_deref()
            .and_then(|v| parse_header("content-security-policy", v))
        {
            router = router.layer(SetResponseHeaderLayer::overriding(
                CONTENT_SECURITY_POLICY.clone(),
                val,
            ));
        }

        if let Some(val) = self
            .referrer_policy
            .as_deref()
            .and_then(|v| parse_header("referrer-policy", v))
        {
            router = router.layer(SetResponseHeaderLayer::overriding(
                REFERRER_POLICY.clone(),
                val,
            ));
        }

        router
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_header_values_are_reported_as_none() {
        assert!(parse_header("x-frame-options", "DENY").is_some());
        assert!(parse_header("x-frame-options", "bad\nvalue").is_none());
    }

    #[test]
    fn default_security_headers_config() {
        let config = SecurityHeadersConfig::default();
        assert_eq!(config.content_type_options, Some("nosniff".to_string()));
        assert_eq!(config.frame_options, Some("DENY".to_string()));
        assert_eq!(config.xss_protection, Some("0".to_string()));
        assert_eq!(
            config.hsts,
            Some("max-age=31536000; includeSubDomains".to_string())
        );
        assert_eq!(config.content_security_policy, None);
        assert_eq!(
            config.referrer_policy,
            Some("strict-origin-when-cross-origin".to_string())
        );
    }
}
