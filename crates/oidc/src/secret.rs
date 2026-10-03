use serde::{Deserialize, Serialize};

/// Secret credentials for a single OIDC provider.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OidcProviderSecret {
    /// Provider name matching OidcProviderConfig.name (e.g. "google", "github").
    pub name: String,
    /// Client secret issued by the OIDC identity provider.
    pub client_secret: String,
}

impl std::fmt::Debug for OidcProviderSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcProviderSecret")
            .field("name", &self.name)
            .field("client_secret", &"[REDACTED]")
            .finish()
    }
}

/// Secret credentials for OpenID Connect (OIDC) client authentication supporting multiple providers.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct OidcSecret {
    /// Optional fallback client secret.
    #[serde(default)]
    pub client_secret: Option<String>,

    /// Provider-specific client secrets.
    #[serde(default)]
    pub providers: Vec<OidcProviderSecret>,
}

impl std::fmt::Debug for OidcSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcSecret")
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("providers", &self.providers)
            .finish()
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OidcSecretDeHelper {
    Multi {
        #[serde(default)]
        client_secret: Option<String>,
        providers: Vec<OidcProviderSecret>,
    },
    Single {
        client_secret: String,
    },
    Empty {},
}

impl<'de> Deserialize<'de> for OidcSecret {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let helper = OidcSecretDeHelper::deserialize(deserializer)?;
        match helper {
            OidcSecretDeHelper::Multi {
                client_secret,
                providers,
            } => Ok(Self {
                client_secret,
                providers,
            }),
            OidcSecretDeHelper::Single { client_secret } => Ok(Self {
                client_secret: Some(client_secret.clone()),
                providers: vec![OidcProviderSecret {
                    name: "google".to_string(),
                    client_secret,
                }],
            }),
            OidcSecretDeHelper::Empty {} => Ok(Self::default()),
        }
    }
}

impl Default for OidcSecret {
    fn default() -> Self {
        Self {
            client_secret: Some("default-oidc-client-secret".to_string()),
            providers: vec![OidcProviderSecret {
                name: "google".to_string(),
                client_secret: "default-oidc-client-secret".to_string(),
            }],
        }
    }
}

impl OidcSecret {
    /// Creates a new single-secret OidcSecret instance.
    pub fn new(client_secret: impl Into<String>) -> Self {
        let sec = client_secret.into();
        Self {
            client_secret: Some(sec.clone()),
            providers: vec![OidcProviderSecret {
                name: "google".to_string(),
                client_secret: sec,
            }],
        }
    }

    /// Creates an OidcSecret instance with multiple provider credentials.
    pub fn with_providers(providers: Vec<OidcProviderSecret>) -> Self {
        Self {
            client_secret: None,
            providers,
        }
    }

    /// Resolves the client secret for a given provider name.
    pub fn get_secret(&self, provider_name: &str) -> Option<&str> {
        let clean = provider_name.trim().to_lowercase();
        self.providers
            .iter()
            .find(|p| p.name.to_lowercase() == clean)
            .map(|p| p.client_secret.as_str())
            .or(self.client_secret.as_deref())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn debug_redacts_client_secret() {
        let secret = OidcSecret::new("super_secret_oidc_key");
        let debug_str = format!("{secret:?}");
        assert!(!debug_str.contains("super_secret_oidc_key"));
        assert!(debug_str.contains("[REDACTED]"));
    }

    #[test]
    fn deserializes_legacy_secret() {
        let json = r#"{"client_secret": "my-secret-123"}"#;
        #[allow(clippy::unwrap_used)]
        let secret: OidcSecret = serde_json::from_str(json).unwrap();
        assert_eq!(secret.get_secret("google"), Some("my-secret-123"));
        assert_eq!(secret.get_secret("keycloak"), Some("my-secret-123"));
    }

    #[test]
    fn deserializes_multi_provider_secret() {
        let json = r#"{
            "providers": [
                {
                    "name": "google",
                    "client_secret": "google-secret-abc"
                },
                {
                    "name": "keycloak",
                    "client_secret": "keycloak-secret-xyz"
                }
            ]
        }"#;
        #[allow(clippy::unwrap_used)]
        let secret: OidcSecret = serde_json::from_str(json).unwrap();
        assert_eq!(secret.get_secret("google"), Some("google-secret-abc"));
        assert_eq!(secret.get_secret("keycloak"), Some("keycloak-secret-xyz"));
        assert_eq!(secret.get_secret("github"), None);
    }
}
