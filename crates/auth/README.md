# auth

JWT authentication middleware for axum services.

It verifies bearer tokens or cookies with [`jwt`](../jwt), exposes the raw
technical claims, and leaves business actor resolution to the application.

---

## ✨ Features

- 🔐 **Bearer and cookie extraction**: reads `Authorization: Bearer` or a
  configured cookie name.
- 🎫 **Verified claims**: `AuthClaims` exposes `subject`, `issued_at`,
  `expires_at`, and raw `roles` names.
- 🧩 **Axum-native**: apply with `axum::middleware::from_fn_with_state`.
- 🧾 **REST-friendly errors**: returns the standard `ErrorResponse` payload with
  `401 UNAUTHORIZED`.

---

## 🚀 Usage

```rust,ignore
use std::sync::Arc;

use auth::{AuthState, AuthClaims, require_auth};
use axum::{Router, middleware::from_fn_with_state, routing::get};

let auth_state = AuthState::new(Arc::new(jwt_service))
    .with_cookie_name("auth_token");

let router = Router::new()
    .route("/me", get(profile))
    .route_layer(from_fn_with_state(auth_state, require_auth));
```

Handlers read the claims from request extensions:

```rust,ignore
async fn profile(Extension(claims): Extension<AuthClaims>) -> String {
    claims.subject
}
```

`AuthClaims` is technical authentication data. Map `claims.roles` (raw token
role names) into application roles and domain actors in the service layer.

---

## 📜 License

Licensed under the [MIT License](LICENSE).
