use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use url::Url;
use utoipa::ToSchema;

use crate::claims::{IdTokenClaims, OidcUserClaims};
use crate::config::OidcConfig;
use crate::error::OidcError;

/// Summary information for an available OIDC identity provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct OidcProviderSummary {
    /// Unique provider identifier (e.g. "google", "github", "keycloak").
    #[schema(example = "google")]
    pub name: String,

    /// Human-readable display name (e.g. "Google", "GitHub Enterprise").
    #[schema(example = "Google")]
    pub display_name: String,

    /// Whether this provider is configured as the default provider.
    #[schema(example = true)]
    pub is_default: bool,
}

/// Raw token response from OIDC token endpoint.
#[derive(Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: Option<String>,
    pub token_type: Option<String>,
    pub id_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    pub scope: Option<String>,
}

impl std::fmt::Debug for TokenResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenResponse")
            .field(
                "access_token",
                &self.access_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("token_type", &self.token_type)
            .field("id_token", &self.id_token.as_ref().map(|_| "[REDACTED]"))
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("expires_in", &self.expires_in)
            .field("scope", &self.scope)
            .finish()
    }
}

/// Service for generating OIDC authorization requests and exchanging authorization codes for verified claims.
#[derive(Clone)]
pub struct OidcService {
    config: OidcConfig,
    client: Client,
}

impl std::fmt::Debug for OidcService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcService")
            .field("config", &self.config)
            .finish()
    }
}

impl OidcService {
    /// Creates a new OidcService instance.
    pub fn new(config: OidcConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    /// Returns the active OIDC configuration.
    pub fn config(&self) -> &OidcConfig {
        &self.config
    }

    /// Returns a list of summaries for all configured OIDC providers.
    pub fn list_providers(&self) -> Vec<OidcProviderSummary> {
        let default_name = self.config.default_provider_name();
        self.config
            .providers()
            .iter()
            .map(|p| {
                let is_default = default_name.is_some_and(|d| d.eq_ignore_ascii_case(&p.name));
                OidcProviderSummary {
                    name: p.name.clone(),
                    display_name: p.display_name().to_string(),
                    is_default,
                }
            })
            .collect()
    }

    /// Generates authorization URL, PKCE code verifier, nonce, and resolved provider name for initiating OIDC flow.
    pub fn start_auth(
        &self,
        provider_name: Option<&str>,
        redirect_uri_override: Option<&str>,
    ) -> Result<(Url, String, String, String), OidcError> {
        let provider = self.config.get_provider(provider_name)?;

        let mut rng = rand::rng();

        // 1. Generate 32-byte cryptographic random nonce
        let mut nonce_bytes = [0u8; 32];
        rng.fill(&mut nonce_bytes);
        let nonce = URL_SAFE_NO_PAD.encode(nonce_bytes);

        // 2. Generate 32-byte PKCE code verifier
        let mut verifier_bytes = [0u8; 32];
        rng.fill(&mut verifier_bytes);
        let code_verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);

        // 3. Compute PKCE S256 code challenge
        let mut hasher = Sha256::new();
        hasher.update(code_verifier.as_bytes());
        let code_challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

        // 4. Determine redirect URI
        let redirect_uri = match redirect_uri_override {
            Some(uri) => uri.to_string(),
            None => provider.redirect_uri.to_string(),
        };

        // 5. Build authorization URL
        let mut auth_url = provider.auth_url();
        {
            let mut pairs = auth_url.query_pairs_mut();
            pairs.append_pair("client_id", &provider.client_id);
            pairs.append_pair("response_type", "code");
            pairs.append_pair("scope", &provider.scopes.join(" "));
            pairs.append_pair("redirect_uri", &redirect_uri);
            pairs.append_pair("nonce", &nonce);
            pairs.append_pair("code_challenge", &code_challenge);
            pairs.append_pair("code_challenge_method", "S256");
            if let Some(prompt) = &provider.prompt {
                pairs.append_pair("prompt", prompt);
            }
        }

        Ok((auth_url, nonce, code_verifier, provider.name.clone()))
    }

    /// Exchanges authorization code with identity provider and returns verified user identity claims.
    pub async fn exchange_code(
        &self,
        provider_name: &str,
        code: &str,
        code_verifier: &str,
        redirect_uri: &str,
        expected_nonce: &str,
        client_secret: &str,
    ) -> Result<OidcUserClaims, OidcError> {
        let provider = self.config.get_provider(Some(provider_name))?;
        let token_url = provider.token_url();

        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", &provider.client_id),
            ("client_secret", client_secret),
            ("code_verifier", code_verifier),
        ];

        let response = self
            .client
            .post(token_url.as_str())
            .form(&params)
            .send()
            .await
            .map_err(|e| OidcError::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable body>".to_string());
            return Err(OidcError::TokenExchange(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let token_res: TokenResponse = response.json().await.map_err(|e| {
            OidcError::TokenExchange(format!("Failed to parse token response: {e}"))
        })?;

        let mut user_claims = if let Some(id_token) = &token_res.id_token {
            Some(self.decode_and_validate_id_token(&provider.name, id_token, expected_nonce)?)
        } else {
            None
        };

        // If ID token did not provide avatar picture or name (or if ID token was absent),
        // query the UserInfo endpoint using the access_token to enrich profile claims.
        if let Some(access_token) = &token_res.access_token {
            let needs_userinfo = match &user_claims {
                Some(claims) => claims.picture.is_none() || claims.name.is_none(),
                None => true,
            };

            if needs_userinfo {
                let userinfo_url = provider.userinfo_url();
                if let Ok(userinfo_res) = self
                    .client
                    .get(userinfo_url.as_str())
                    .bearer_auth(access_token)
                    .send()
                    .await
                    && userinfo_res.status().is_success()
                    && let Ok(info_claims) = userinfo_res.json::<IdTokenClaims>().await
                {
                    if let Some(ref mut claims) = user_claims {
                        if claims.picture.is_none() {
                            claims.picture = info_claims.picture;
                        }
                        if claims.name.is_none() {
                            claims.name = info_claims.name;
                        }
                        if claims.given_name.is_none() {
                            claims.given_name = info_claims.given_name;
                        }
                        if claims.family_name.is_none() {
                            claims.family_name = info_claims.family_name;
                        }
                        if claims.locale.is_none() {
                            claims.locale = info_claims.locale;
                        }
                    } else if let Ok(parsed_claims) = info_claims.into_user_claims(&provider.name) {
                        user_claims = Some(parsed_claims);
                    }
                }
            }
        }

        user_claims.ok_or_else(|| {
            OidcError::MissingClaim("Neither ID token nor valid UserInfo received".to_string())
        })
    }

    /// Decodes an ID token (payload segment), checks expiry and nonce matching, and parses user claims.
    pub fn decode_and_validate_id_token(
        &self,
        provider_name: &str,
        id_token: &str,
        expected_nonce: &str,
    ) -> Result<OidcUserClaims, OidcError> {
        let parts: Vec<&str> = id_token.split('.').collect();
        if parts.len() < 2 {
            return Err(OidcError::InvalidIdToken(
                "ID token must have at least header and payload segments".to_string(),
            ));
        }

        let payload_bytes = URL_SAFE_NO_PAD
            .decode(parts[1])
            .or_else(|_| base64::engine::general_purpose::STANDARD.decode(parts[1]))
            .map_err(|e| {
                OidcError::InvalidIdToken(format!("Failed to base64-decode token payload: {e}"))
            })?;

        let raw_claims: IdTokenClaims = serde_json::from_slice(&payload_bytes).map_err(|e| {
            OidcError::InvalidIdToken(format!("Failed to parse ID token JSON: {e}"))
        })?;

        // Verify nonce to prevent token replay attacks
        if let Some(token_nonce) = &raw_claims.nonce
            && token_nonce != expected_nonce
        {
            return Err(OidcError::SecurityVerification(format!(
                "Nonce mismatch: expected '{expected_nonce}', got '{token_nonce}'"
            )));
        }

        // Verify expiration
        let now = OffsetDateTime::now_utc().unix_timestamp() as u64;
        if raw_claims.is_expired(now) {
            return Err(OidcError::InvalidIdToken(
                "ID token has expired".to_string(),
            ));
        }

        raw_claims
            .into_user_claims(provider_name)
            .map_err(OidcError::MissingClaim)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::config::OidcProviderConfig;

    #[test]
    #[allow(clippy::unwrap_used)]
    fn generates_authorization_url_with_pkce_and_nonce() {
        let config = OidcConfig::default();
        let service = OidcService::new(config);

        let (url, nonce, verifier, provider_name) = service.start_auth(None, None).unwrap();
        assert_eq!(provider_name, "google");
        assert!(!nonce.is_empty());
        assert!(!verifier.is_empty());

        let url_str = url.as_str();
        assert!(url_str.contains("response_type=code"));
        assert!(url_str.contains("code_challenge_method=S256"));
        assert!(url_str.contains("code_challenge="));
        assert!(url_str.contains(&format!("nonce={nonce}")));
        assert!(url_str.contains("client_id=default-oidc-client-id"));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn starts_auth_for_specific_provider_in_multi_provider_setup() {
        let google = OidcProviderConfig::default();
        let keycloak = OidcProviderConfig {
            name: "keycloak".to_string(),
            display_name: Some("Keycloak SSO".to_string()),
            client_id: "keycloak-client-id".to_string(),
            issuer_url: Url::parse("https://auth.example.com/realms/master").unwrap(),
            auth_url: None,
            token_url: None,
            userinfo_url: None,
            jwks_url: None,
            redirect_uri: Url::parse("http://localhost:8080/callback").unwrap(),
            scopes: vec!["openid".to_string(), "email".to_string()],
            prompt: None,
        };

        let config = OidcConfig::new(Some("google".to_string()), vec![google, keycloak]);
        let service = OidcService::new(config);

        let (url, _nonce, _verifier, provider_name) =
            service.start_auth(Some("keycloak"), None).unwrap();
        assert_eq!(provider_name, "keycloak");
        assert!(url.as_str().contains("auth.example.com"));
        assert!(url.as_str().contains("client_id=keycloak-client-id"));

        let providers = service.list_providers();
        assert_eq!(providers.len(), 2);
        assert_eq!(providers[0].name, "google");
        assert!(providers[0].is_default);
        assert_eq!(providers[1].name, "keycloak");
        assert!(!providers[1].is_default);
    }

    #[test]
    fn rejects_unknown_provider_start_auth() {
        let config = OidcConfig::default();
        let service = OidcService::new(config);

        let err = service.start_auth(Some("unknown-provider"), None);
        assert!(matches!(err, Err(OidcError::ProviderNotFound(_))));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn decodes_and_validates_id_token() {
        let config = OidcConfig::default();
        let service = OidcService::new(config);

        let header = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9";
        let future_exp = (OffsetDateTime::now_utc().unix_timestamp() + 3600) as u64;

        let payload_json = serde_json::json!({
            "iss": "https://accounts.google.com",
            "sub": "google-user-123",
            "email": "jane@example.com",
            "email_verified": true,
            "name": "Jane Doe",
            "given_name": "Jane",
            "family_name": "Doe",
            "picture": "https://example.com/avatar.png",
            "nonce": "test_nonce_123",
            "exp": future_exp
        });

        let payload_b64 = URL_SAFE_NO_PAD.encode(payload_json.to_string());
        let mock_token = format!("{header}.{payload_b64}.mock_signature");

        let claims = service
            .decode_and_validate_id_token("google", &mock_token, "test_nonce_123")
            .unwrap();

        assert_eq!(claims.sub, "google-user-123");
        assert_eq!(claims.email, "jane@example.com");
        assert_eq!(claims.name.as_deref(), Some("Jane Doe"));
        assert_eq!(claims.provider, "google");
    }

    #[test]
    fn rejects_id_token_with_nonce_mismatch() {
        let config = OidcConfig::default();
        let service = OidcService::new(config);

        let header = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9";
        let future_exp = (OffsetDateTime::now_utc().unix_timestamp() + 3600) as u64;

        let payload_json = serde_json::json!({
            "iss": "https://accounts.google.com",
            "sub": "google-user-123",
            "email": "jane@example.com",
            "nonce": "attacker_nonce",
            "exp": future_exp
        });

        let payload_b64 = URL_SAFE_NO_PAD.encode(payload_json.to_string());
        let mock_token = format!("{header}.{payload_b64}.mock_signature");

        let result =
            service.decode_and_validate_id_token("google", &mock_token, "my_expected_nonce");
        assert!(matches!(result, Err(OidcError::SecurityVerification(_))));
    }

    #[test]
    fn rejects_expired_id_token() {
        let config = OidcConfig::default();
        let service = OidcService::new(config);

        let header = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9";
        let past_exp = (OffsetDateTime::now_utc().unix_timestamp() - 3600) as u64;

        let payload_json = serde_json::json!({
            "iss": "https://accounts.google.com",
            "sub": "google-user-123",
            "email": "jane@example.com",
            "nonce": "my_nonce",
            "exp": past_exp
        });

        let payload_b64 = URL_SAFE_NO_PAD.encode(payload_json.to_string());
        let mock_token = format!("{header}.{payload_b64}.mock_signature");

        let result = service.decode_and_validate_id_token("google", &mock_token, "my_nonce");
        assert!(matches!(result, Err(OidcError::InvalidIdToken(_))));
    }

    #[test]
    fn debug_redacts_tokens() {
        let resp = TokenResponse {
            access_token: Some("secret_access_token_123".to_string()),
            token_type: Some("Bearer".to_string()),
            id_token: Some("secret_id_token_456".to_string()),
            refresh_token: Some("secret_refresh_token_789".to_string()),
            expires_in: Some(3600),
            scope: Some("openid email".to_string()),
        };
        let debug_str = format!("{resp:?}");
        assert!(!debug_str.contains("secret_access_token_123"));
        assert!(!debug_str.contains("secret_id_token_456"));
        assert!(!debug_str.contains("secret_refresh_token_789"));
        assert!(debug_str.contains("[REDACTED]"));
    }
}
