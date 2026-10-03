use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Verified user identity claims extracted from an OIDC ID token and/or UserInfo endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct OidcUserClaims {
    /// Unique subject identifier within the identity provider.
    #[schema(example = "109876543210987654321")]
    pub sub: String,

    /// User's email address.
    #[schema(example = "jane.doe@example.com")]
    pub email: String,

    /// Whether the email address has been verified by the provider.
    pub email_verified: Option<bool>,

    /// Full display name.
    #[schema(example = "Jane Doe")]
    pub name: Option<String>,

    /// Given / first name.
    #[schema(example = "Jane")]
    pub given_name: Option<String>,

    /// Family / last name.
    #[schema(example = "Doe")]
    pub family_name: Option<String>,

    /// Avatar / profile picture URL.
    #[schema(example = "https://lh3.googleusercontent.com/a/default-user")]
    pub picture: Option<String>,

    /// Preferred locale (e.g. "en-US", "vi-VN").
    #[schema(example = "en-US")]
    pub locale: Option<String>,

    /// Identifier of the identity provider (e.g. "google", "keycloak").
    #[schema(example = "google")]
    pub provider: String,
}

/// Standard raw ID Token payload claims as defined in OpenID Connect Core 1.0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdTokenClaims {
    pub iss: String,
    pub sub: String,
    #[serde(default)]
    pub aud: serde_json::Value,
    #[serde(default)]
    pub exp: Option<u64>,
    #[serde(default)]
    pub iat: Option<u64>,
    #[serde(default)]
    pub nonce: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: Option<bool>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub given_name: Option<String>,
    #[serde(default)]
    pub family_name: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub preferred_username: Option<String>,
}

impl IdTokenClaims {
    /// Checks if the ID token has expired at the given timestamp.
    pub fn is_expired(&self, now_timestamp: u64) -> bool {
        if let Some(exp) = self.exp {
            now_timestamp >= exp
        } else {
            false
        }
    }

    /// Converts raw ID token claims into verified [`OidcUserClaims`].
    pub fn into_user_claims(self, provider: &str) -> Result<OidcUserClaims, String> {
        let email = self.email.or(self.preferred_username).ok_or_else(|| {
            "ID token contains neither 'email' nor 'preferred_username'".to_string()
        })?;

        Ok(OidcUserClaims {
            sub: self.sub,
            email,
            email_verified: self.email_verified,
            name: self.name,
            given_name: self.given_name,
            family_name: self.family_name,
            picture: self.picture,
            locale: self.locale,
            provider: provider.to_string(),
        })
    }
}
