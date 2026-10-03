use thiserror::Error;

/// Errors that can occur during OpenID Connect authentication ceremonies.
#[derive(Debug, Error)]
pub enum OidcError {
    /// Requested identity provider was not found in configuration.
    #[error("OIDC provider '{0}' not found in configuration")]
    ProviderNotFound(String),

    /// Invalid configuration or URL.
    #[error("Invalid OIDC URL: {0}")]
    InvalidUrl(String),

    /// HTTP request failure while communicating with the OIDC identity provider.
    #[error("OIDC HTTP request failed: {0}")]
    Http(String),

    /// Failure during authorization code exchange.
    #[error("OIDC token exchange failed: {0}")]
    TokenExchange(String),

    /// Invalid or malformed ID token returned by the identity provider.
    #[error("Invalid OIDC ID token: {0}")]
    InvalidIdToken(String),

    /// Missing mandatory claims (e.g. email or subject) from identity provider.
    #[error("OIDC response is missing required claim: {0}")]
    MissingClaim(String),

    /// Nonce or state mismatch detected (potential CSRF or replay attack).
    #[error("OIDC security verification failed: {0}")]
    SecurityVerification(String),

    /// Serialization or deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
