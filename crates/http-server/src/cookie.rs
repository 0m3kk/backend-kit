//! Cookie configuration, utilities, and extractors.

pub use axum_extra::extract::cookie::{Cookie, CookieJar, Key, PrivateCookieJar, SignedCookieJar};
use cookie::SameSite as CookieSameSite;
use serde::{Deserialize, Serialize};
use time::Duration;

/// SameSite policy for cookies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SameSite {
    /// Lax policy (recommended default for top-level navigations).
    #[default]
    Lax,
    /// Strict policy (same-origin only).
    Strict,
    /// None policy (cross-site allowed, requires Secure).
    None,
}

impl From<SameSite> for CookieSameSite {
    fn from(s: SameSite) -> Self {
        match s {
            SameSite::Lax => CookieSameSite::Lax,
            SameSite::Strict => CookieSameSite::Strict,
            SameSite::None => CookieSameSite::None,
        }
    }
}

/// Cookie configuration parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CookieConfig {
    /// Domain for cookies.
    #[serde(default)]
    pub domain: Option<String>,

    /// Path for cookies.
    #[serde(default = "default_cookie_path")]
    pub path: String,

    /// Whether cookies require HTTPS.
    #[serde(default)]
    pub secure: bool,

    /// Whether cookies are inaccessible to client-side scripts.
    #[serde(default = "default_http_only")]
    pub http_only: bool,

    /// SameSite policy for cookies (`lax`, `strict`, `none`).
    #[serde(default)]
    pub same_site: SameSite,

    /// Maximum lifetime in seconds for cookies.
    #[serde(default)]
    pub max_age_secs: Option<u64>,
}

fn default_cookie_path() -> String {
    "/".to_string()
}

const fn default_http_only() -> bool {
    true
}

impl Default for CookieConfig {
    fn default() -> Self {
        Self {
            domain: None,
            path: default_cookie_path(),
            secure: false,
            http_only: default_http_only(),
            same_site: SameSite::default(),
            max_age_secs: None,
        }
    }
}

impl CookieConfig {
    /// Creates a [`Cookie`] with configured domain, path, secure, http_only, and same_site attributes.
    pub fn build_cookie<'a>(
        &self,
        name: impl Into<std::borrow::Cow<'a, str>>,
        value: impl Into<std::borrow::Cow<'a, str>>,
    ) -> Cookie<'a> {
        let mut builder = Cookie::build((name, value))
            .path(self.path.clone())
            .http_only(self.http_only)
            .same_site(CookieSameSite::from(self.same_site));

        if self.secure {
            builder = builder.secure(true);
        }
        if let Some(ref domain) = self.domain {
            builder = builder.domain(domain.clone());
        }
        if let Some(max_age) = self.max_age_secs {
            builder = builder.max_age(Duration::seconds(max_age as i64));
        }

        builder.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cookie_config() {
        let config = CookieConfig::default();
        assert_eq!(config.path, "/");
        assert!(config.http_only);
        assert!(!config.secure);
        assert_eq!(config.same_site, SameSite::Lax);
        assert_eq!(config.domain, None);
        assert_eq!(config.max_age_secs, None);
    }

    #[test]
    fn build_cookie_uses_config_attributes() {
        let config = CookieConfig {
            domain: Some("example.com".to_string()),
            path: "/api".to_string(),
            secure: true,
            http_only: true,
            same_site: SameSite::Strict,
            max_age_secs: Some(3600),
        };

        let cookie = config.build_cookie("token", "12345");
        assert_eq!(cookie.name(), "token");
        assert_eq!(cookie.value(), "12345");
        assert_eq!(cookie.path(), Some("/api"));
        assert_eq!(cookie.domain(), Some("example.com"));
        assert_eq!(cookie.http_only(), Some(true));
        assert_eq!(cookie.secure(), Some(true));
        assert_eq!(cookie.same_site(), Some(CookieSameSite::Strict));
        assert_eq!(cookie.max_age(), Some(Duration::seconds(3600)));
    }
}
