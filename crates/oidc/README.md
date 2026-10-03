# 🔑 oidc

Production-ready OpenID Connect (OIDC) authentication service, configuration, PKCE flow, and token validation for Rust applications.

---

## ✨ Features

- 🔐 **Authorization Code Flow + PKCE**: Full RFC 7636 Proof Key for Code Exchange support using S256 challenge generation.
- 🌐 **Provider Agnostic**: Seamless integration with Google, GitHub, Auth0, Okta, Keycloak, or any standard OIDC identity provider.
- 🛡️ **Stateless CSRF & Nonce Protection**: Generates cryptographically secure state tokens and nonces to prevent CSRF and replay attacks without server-side session stores.
- 🏷️ **ID Token Validation**: Parse and validate standard OIDC ID token claims ([`OidcClaims`]) including `sub`, `email`, `name`, `picture`, and `email_verified`.
- 📑 **OpenAPI Integration**: Schema generation with `utoipa` out-of-the-box.

---

## 🚀 Quick Start

### 1. Add Dependency

Add `oidc` to your `Cargo.toml`:

```toml
[dependencies]
oidc = "0.1"
```

### 2. Usage Example

```rust
use oidc::{OidcConfig, OidcSecret, OidcService};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize configuration and secrets
    let config = OidcConfig::default();
    let secret = OidcSecret::default();
    let service = OidcService::new(&config, &secret)?;

    // 2. Generate authorization URL and PKCE verifier
    let auth_request = service.create_authorization_url("state_token")?;
    println!("Redirect user to: {}", auth_request.url);

    // 3. Exchange authorization code for claims
    let claims = service
        .exchange_code_for_claims("auth_code", &auth_request.pkce_verifier, Some("nonce"))
        .await?;

    println!("Authenticated user: {} ({})", claims.sub, claims.email.unwrap_or_default());

    Ok(())
}
```

---

## 📜 License

Licensed under the [MIT License](LICENSE).
