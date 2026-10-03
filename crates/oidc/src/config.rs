use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::OidcError;

fn default_oidc_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "email".to_string(),
        "profile".to_string(),
    ]
}

/// Configuration for a single OpenID Connect (OIDC) Identity Provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OidcProviderConfig {
    /// Human-readable provider identifier (e.g. "google", "github", "keycloak", "azure").
    #[serde(alias = "provider_name")]
    pub name: String,

    /// Optional human-readable display name (e.g. "Google", "GitHub Enterprise", "Keycloak SSO").
    #[serde(default)]
    pub display_name: Option<String>,

    /// Client ID issued by the OIDC Identity Provider.
    pub client_id: String,

    /// Issuer URL of the OIDC Identity Provider (e.g. "https://accounts.google.com").
    pub issuer_url: Url,

    /// Authorization endpoint URL. If omitted, constructed from issuer_url.
    #[serde(default)]
    pub auth_url: Option<Url>,

    /// Token exchange endpoint URL. If omitted, constructed from issuer_url.
    #[serde(default)]
    pub token_url: Option<Url>,

    /// UserInfo endpoint URL. If omitted, constructed from issuer_url.
    #[serde(default)]
    pub userinfo_url: Option<Url>,

    /// JSON Web Key Set (JWKS) URL for public key discovery.
    #[serde(default)]
    pub jwks_url: Option<Url>,

    /// Default callback / redirect URI registered with the IdP.
    pub redirect_uri: Url,

    /// OIDC scopes requested during authorization.
    #[serde(default = "default_oidc_scopes")]
    pub scopes: Vec<String>,

    /// Optional prompt parameter for OAuth2/OIDC provider (e.g. "select_account", "consent").
    #[serde(default)]
    pub prompt: Option<String>,
}

impl Default for OidcProviderConfig {
    fn default() -> Self {
        #[allow(clippy::expect_used)]
        let issuer_url =
            Url::parse("https://accounts.google.com").expect("default OIDC issuer must be valid");
        #[allow(clippy::expect_used)]
        let auth_url = Url::parse("https://accounts.google.com/o/oauth2/v2/auth")
            .expect("default OIDC auth URL must be valid");
        #[allow(clippy::expect_used)]
        let token_url = Url::parse("https://oauth2.googleapis.com/token")
            .expect("default OIDC token URL must be valid");
        #[allow(clippy::expect_used)]
        let userinfo_url = Url::parse("https://openidconnect.googleapis.com/v1/userinfo")
            .expect("default OIDC userinfo URL must be valid");
        #[allow(clippy::expect_used)]
        let jwks_url = Url::parse("https://www.googleapis.com/oauth2/v3/certs")
            .expect("default OIDC jwks URL must be valid");
        #[allow(clippy::expect_used)]
        let redirect_uri = Url::parse("http://localhost:8080/api/v1/auth/oidc/callback")
            .expect("default OIDC redirect URI must be valid");

        Self {
            name: "google".to_string(),
            display_name: Some("Google".to_string()),
            client_id: "default-oidc-client-id".to_string(),
            issuer_url,
            auth_url: Some(auth_url),
            token_url: Some(token_url),
            userinfo_url: Some(userinfo_url),
            jwks_url: Some(jwks_url),
            redirect_uri,
            scopes: default_oidc_scopes(),
            prompt: Some("select_account".to_string()),
        }
    }
}

impl OidcProviderConfig {
    /// Returns the display name of this provider, falling back to its `name`.
    pub fn display_name(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.name)
    }

    /// Returns the effective authorization URL.
    pub fn auth_url(&self) -> Url {
        if let Some(auth_url) = &self.auth_url {
            auth_url.clone()
        } else {
            let mut url = self.issuer_url.clone();
            if let Ok(mut segments) = url.path_segments_mut() {
                segments
                    .push("protocol")
                    .push("openid-connect")
                    .push("auth");
            }
            url
        }
    }

    /// Returns the effective token endpoint URL.
    pub fn token_url(&self) -> Url {
        if let Some(token_url) = &self.token_url {
            token_url.clone()
        } else {
            let mut url = self.issuer_url.clone();
            if let Ok(mut segments) = url.path_segments_mut() {
                segments
                    .push("protocol")
                    .push("openid-connect")
                    .push("token");
            }
            url
        }
    }

    /// Returns the effective userinfo endpoint URL.
    pub fn userinfo_url(&self) -> Url {
        if let Some(userinfo_url) = &self.userinfo_url {
            userinfo_url.clone()
        } else {
            let mut url = self.issuer_url.clone();
            if let Ok(mut segments) = url.path_segments_mut() {
                segments
                    .push("protocol")
                    .push("openid-connect")
                    .push("userinfo");
            }
            url
        }
    }
}

/// Configuration for OpenID Connect (OIDC) relying party supporting multiple providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OidcConfig {
    /// Default provider identifier to use when no specific provider is requested.
    #[serde(default)]
    pub default_provider: Option<String>,

    /// Configured OIDC identity providers.
    #[serde(default)]
    pub providers: Vec<OidcProviderConfig>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OidcConfigDeHelper {
    Multi {
        #[serde(default)]
        default_provider: Option<String>,
        providers: Vec<OidcProviderConfig>,
    },
    Single(Box<OidcProviderConfig>),
    Empty {},
}

impl<'de> Deserialize<'de> for OidcConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let helper = OidcConfigDeHelper::deserialize(deserializer)?;
        match helper {
            OidcConfigDeHelper::Multi {
                default_provider,
                providers,
            } => {
                let default_provider =
                    default_provider.or_else(|| providers.first().map(|p| p.name.clone()));
                Ok(Self {
                    default_provider,
                    providers,
                })
            }
            OidcConfigDeHelper::Single(single) => Ok(Self {
                default_provider: Some(single.name.clone()),
                providers: vec![*single],
            }),
            OidcConfigDeHelper::Empty {} => Ok(Self::default()),
        }
    }
}

impl Default for OidcConfig {
    fn default() -> Self {
        Self {
            default_provider: Some("google".to_string()),
            providers: vec![OidcProviderConfig::default()],
        }
    }
}

impl OidcConfig {
    /// Creates a new `OidcConfig` with an optional default provider and list of providers.
    pub fn new(default_provider: Option<String>, providers: Vec<OidcProviderConfig>) -> Self {
        Self {
            default_provider,
            providers,
        }
    }

    /// Looks up a provider configuration by name (case-insensitive).
    /// If `name` is None, attempts to resolve the default provider.
    pub fn get_provider(&self, name: Option<&str>) -> Result<&OidcProviderConfig, OidcError> {
        if let Some(target) = name {
            let target_clean = target.trim().to_lowercase();
            if target_clean.is_empty() {
                return self.get_provider(None);
            }
            self.providers
                .iter()
                .find(|p| p.name.to_lowercase() == target_clean)
                .ok_or_else(|| OidcError::ProviderNotFound(target.to_string()))
        } else if let Some(default_name) = &self.default_provider {
            let def_clean = default_name.trim().to_lowercase();
            self.providers
                .iter()
                .find(|p| p.name.to_lowercase() == def_clean)
                .or_else(|| self.providers.first())
                .ok_or_else(|| OidcError::ProviderNotFound(default_name.clone()))
        } else {
            self.providers
                .first()
                .ok_or_else(|| OidcError::ProviderNotFound("default".to_string()))
        }
    }

    /// Returns the slice of all configured providers.
    pub fn providers(&self) -> &[OidcProviderConfig] {
        &self.providers
    }

    /// Returns the default provider name if configured.
    pub fn default_provider_name(&self) -> Option<&str> {
        self.default_provider
            .as_deref()
            .or_else(|| self.providers.first().map(|p| p.name.as_str()))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let config = OidcConfig::default();
        assert_eq!(config.default_provider.as_deref(), Some("google"));
        assert_eq!(config.providers.len(), 1);

        let provider = config.get_provider(None).unwrap();
        assert_eq!(provider.name, "google");
        assert_eq!(provider.client_id, "default-oidc-client-id");
        assert_eq!(provider.issuer_url.as_str(), "https://accounts.google.com/");
        assert_eq!(provider.scopes, vec!["openid", "email", "profile"]);
    }

    #[test]
    fn deserializes_legacy_single_config() {
        let json = r#"{
            "provider_name": "keycloak",
            "client_id": "custom-client",
            "issuer_url": "https://auth.example.com/realms/master",
            "redirect_uri": "http://localhost:3000/callback",
            "scopes": ["openid", "email"]
        }"#;
        #[allow(clippy::unwrap_used)]
        let config: OidcConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.default_provider.as_deref(), Some("keycloak"));
        assert_eq!(config.providers.len(), 1);

        let provider = config.get_provider(Some("keycloak")).unwrap();
        assert_eq!(provider.name, "keycloak");
        assert_eq!(provider.client_id, "custom-client");
        assert_eq!(
            provider.issuer_url.as_str(),
            "https://auth.example.com/realms/master"
        );
        assert_eq!(
            provider.redirect_uri.as_str(),
            "http://localhost:3000/callback"
        );
        assert_eq!(provider.scopes, vec!["openid", "email"]);
    }

    #[test]
    fn deserializes_multi_provider_config() {
        let json = r#"{
            "default_provider": "keycloak",
            "providers": [
                {
                    "name": "google",
                    "display_name": "Google",
                    "client_id": "google-client-123",
                    "issuer_url": "https://accounts.google.com",
                    "redirect_uri": "http://localhost:8080/api/v1/auth/oidc/callback",
                    "scopes": ["openid", "email"]
                },
                {
                    "name": "keycloak",
                    "display_name": "Keycloak SSO",
                    "client_id": "keycloak-client-456",
                    "issuer_url": "https://sso.example.com/realms/main",
                    "redirect_uri": "http://localhost:8080/api/v1/auth/oidc/callback"
                }
            ]
        }"#;
        #[allow(clippy::unwrap_used)]
        let config: OidcConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.default_provider.as_deref(), Some("keycloak"));
        assert_eq!(config.providers.len(), 2);

        let google = config.get_provider(Some("google")).unwrap();
        assert_eq!(google.name, "google");
        assert_eq!(google.display_name(), "Google");
        assert_eq!(google.client_id, "google-client-123");

        let default_prov = config.get_provider(None).unwrap();
        assert_eq!(default_prov.name, "keycloak");
        assert_eq!(default_prov.display_name(), "Keycloak SSO");

        let not_found = config.get_provider(Some("github"));
        assert!(matches!(not_found, Err(OidcError::ProviderNotFound(_))));
    }

    #[test]
    fn default_endpoints_constructed_from_issuer_url() {
        let provider = OidcProviderConfig {
            name: "keycloak".to_string(),
            display_name: None,
            client_id: "test-client".to_string(),
            issuer_url: Url::parse("https://auth.example.com/realms/master").unwrap(),
            auth_url: None,
            token_url: None,
            userinfo_url: None,
            jwks_url: None,
            redirect_uri: Url::parse("http://localhost:8080/cb").unwrap(),
            scopes: default_oidc_scopes(),
            prompt: None,
        };

        assert_eq!(
            provider.auth_url().as_str(),
            "https://auth.example.com/realms/master/protocol/openid-connect/auth"
        );
        assert_eq!(
            provider.token_url().as_str(),
            "https://auth.example.com/realms/master/protocol/openid-connect/token"
        );
        assert_eq!(
            provider.userinfo_url().as_str(),
            "https://auth.example.com/realms/master/protocol/openid-connect/userinfo"
        );
        assert_eq!(provider.display_name(), "keycloak");
    }
}
