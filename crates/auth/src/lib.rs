//! JWT authentication middleware for axum services.
//!
//! [`require_auth`] extracts a bearer token from the `Authorization` header or
//! a configured cookie, verifies it with [`jwt::JwtService`], and inserts the
//! resulting [`AuthClaims`] into request extensions.
//!
//! `AuthClaims` is deliberately technical: it carries the raw token subject,
//! timestamps, and role names. Applications resolve business actors and
//! permissions from those claims in their own layer.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use http_server::{CookieJar, ErrorResponse};
use jwt::{JwtError, JwtService};
use serde::{Deserialize, Serialize};

/// Custom claims carried by application tokens.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthCustomClaims {
    /// Application roles granted to the subject (e.g. `manager`, `viewer`).
    #[serde(default)]
    pub roles: Vec<String>,
}

/// Technical claims extracted from a verified JWT.
///
/// This is authentication data, not a business user. Resolve the business
/// actor separately in the application layer using [`AuthClaims::subject`] and
/// [`AuthClaims::roles`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthClaims {
    /// Subject identifier from the JWT `sub` claim.
    pub subject: String,

    /// Issued-at timestamp (seconds since the Unix epoch).
    pub issued_at: u64,

    /// Expiration timestamp (seconds since the Unix epoch).
    pub expires_at: u64,

    /// Application roles from the JWT `roles` claim.
    pub roles: Vec<String>,
}

impl AuthClaims {
    /// Creates technical claims from a verified JWT.
    pub fn new(
        subject: impl Into<String>,
        issued_at: u64,
        expires_at: u64,
        roles: Vec<String>,
    ) -> Self {
        Self {
            subject: subject.into(),
            issued_at,
            expires_at,
            roles,
        }
    }
}

/// Runtime state required by [`require_auth`].
#[derive(Debug, Clone)]
pub struct AuthState {
    jwt_service: Arc<JwtService>,
    cookie_name: Option<String>,
}

impl AuthState {
    /// Creates auth state backed by a shared JWT service.
    pub fn new(jwt_service: Arc<JwtService>) -> Self {
        Self {
            jwt_service,
            cookie_name: None,
        }
    }

    /// Enables token extraction from a cookie with the given name.
    #[must_use]
    pub fn with_cookie_name(mut self, name: impl Into<String>) -> Self {
        self.cookie_name = Some(name.into());
        self
    }
}

/// Middleware that verifies a JWT from the `Authorization: Bearer` header
/// or a configured cookie, then inserts [`AuthClaims`] into request extensions.
///
/// Apply it with [`axum::middleware::from_fn_with_state`] on protected routes:
///
/// ```rust,ignore
/// let auth_state = AuthState::new(Arc::new(jwt_service))
///     .with_cookie_name("auth_token");
///
/// let router = Router::new()
///     .route("/me", get(profile))
///     .route_layer(from_fn_with_state(auth_state, require_auth));
/// ```
pub async fn require_auth(
    State(state): State<AuthState>,
    jar: CookieJar,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(token) = bearer_token(&request).or_else(|| cookie_token(&state, &jar)) else {
        return unauthorized("Authentication token is missing");
    };

    let claims = match verify_token(&state.jwt_service, &token) {
        Ok(claims) => claims,
        Err(message) => return unauthorized(message),
    };

    request.extensions_mut().insert(claims);

    next.run(request).await
}

fn verify_token(service: &JwtService, token: &str) -> Result<AuthClaims, &'static str> {
    let claims = service
        .verify_token::<AuthCustomClaims>(token)
        .map_err(|error| match error {
            JwtError::ExpiredSignature => "Authentication token has expired",
            _ => "Invalid authentication token",
        })?;

    Ok(AuthClaims::new(
        claims.sub,
        claims.iat,
        claims.exp,
        claims.custom.roles,
    ))
}

fn bearer_token(request: &Request) -> Option<String> {
    let value = request
        .headers()
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .trim();

    let mut parts = value.split_whitespace();
    let scheme = parts.next()?;
    let token = parts.next()?;

    if !scheme.eq_ignore_ascii_case("bearer") || parts.next().is_some() {
        return None;
    }

    Some(token.to_string())
}

fn cookie_token(state: &AuthState, jar: &CookieJar) -> Option<String> {
    let name = state.cookie_name.as_deref()?;
    let value = jar.get(name)?.value();

    (!value.is_empty()).then(|| value.to_string())
}

fn unauthorized(message: &'static str) -> Response {
    ErrorResponse::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", message).into_response()
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{HeaderMap, HeaderValue};
    use jwt::JwtConfig;

    use super::*;

    #[test]
    fn extracts_bearer_token_case_insensitively() -> Result<(), Box<dyn std::error::Error>> {
        let request = Request::builder()
            .header(header::AUTHORIZATION, "bearer token-123")
            .body(Body::empty())?;

        assert_eq!(bearer_token(&request).as_deref(), Some("token-123"));
        Ok(())
    }

    #[test]
    fn rejects_non_bearer_authorization() -> Result<(), Box<dyn std::error::Error>> {
        let request = Request::builder()
            .header(header::AUTHORIZATION, "Basic abc123")
            .body(Body::empty())?;

        assert_eq!(bearer_token(&request), None);
        Ok(())
    }

    #[test]
    fn extracts_configured_cookie_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("auth_token=cookie-token"),
        );
        let jar = CookieJar::from_headers(&headers);
        let state = AuthState::new(Arc::new(JwtService::new("secret", JwtConfig::default())))
            .with_cookie_name("auth_token");

        assert_eq!(cookie_token(&state, &jar).as_deref(), Some("cookie-token"));
    }

    #[test]
    fn ignores_cookie_without_configured_name() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("auth_token=cookie-token"),
        );
        let jar = CookieJar::from_headers(&headers);
        let state = AuthState::new(Arc::new(JwtService::new("secret", JwtConfig::default())));

        assert_eq!(cookie_token(&state, &jar), None);
    }

    #[test]
    fn extracts_roles_from_custom_claims() -> Result<(), Box<dyn std::error::Error>> {
        let service = JwtService::new("secret", JwtConfig::default());
        let token = service.generate_token_with_custom(
            "user-1",
            AuthCustomClaims {
                roles: vec!["manager".to_string()],
            },
        )?;

        let claims = verify_token(&service, &token).map_err(std::io::Error::other)?;

        assert_eq!(claims.subject, "user-1");
        assert_eq!(claims.roles, vec!["manager".to_string()]);
        Ok(())
    }

    #[test]
    fn tokens_without_roles_still_verify() -> Result<(), Box<dyn std::error::Error>> {
        let service = JwtService::new("secret", JwtConfig::default());
        let token = service.generate_token("user-1")?;

        let claims = verify_token(&service, &token).map_err(std::io::Error::other)?;

        assert!(claims.roles.is_empty());
        Ok(())
    }
}
