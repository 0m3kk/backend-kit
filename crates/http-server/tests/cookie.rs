#![allow(clippy::unwrap_used)]

use axum::Router;
use axum::extract::FromRef;
use axum::http::header::HeaderValue;
use axum::http::{Request, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use http_body_util::BodyExt;
use http_server::cookie::{
    Cookie, CookieConfig, CookieJar, Key, PrivateCookieJar, SameSite, SignedCookieJar,
};
use tower::ServiceExt;

#[derive(Clone)]
struct AppState {
    key: Key,
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.key.clone()
    }
}

async fn set_cookie_handler(jar: CookieJar) -> impl IntoResponse {
    jar.add(Cookie::new("session_id", "abc123xyz"))
}

async fn get_cookie_handler(jar: CookieJar) -> impl IntoResponse {
    jar.get("session_id")
        .map(|c| c.value().to_string())
        .unwrap_or_else(|| "none".to_string())
}

async fn set_signed_cookie_handler(jar: SignedCookieJar) -> impl IntoResponse {
    jar.add(Cookie::new("user_id", "user_42"))
}

async fn get_signed_cookie_handler(jar: SignedCookieJar) -> impl IntoResponse {
    jar.get("user_id")
        .map(|c| c.value().to_string())
        .unwrap_or_else(|| "none".to_string())
}

async fn set_private_cookie_handler(jar: PrivateCookieJar) -> impl IntoResponse {
    jar.add(Cookie::new("secret_data", "classified_info"))
}

async fn get_private_cookie_handler(jar: PrivateCookieJar) -> impl IntoResponse {
    jar.get("secret_data")
        .map(|c| c.value().to_string())
        .unwrap_or_else(|| "none".to_string())
}

#[test]
fn cookie_config_builds_cookie_with_attributes() {
    let config = CookieConfig {
        domain: Some("example.com".to_string()),
        path: "/auth".to_string(),
        secure: true,
        http_only: true,
        same_site: SameSite::Strict,
        max_age_secs: Some(3600),
    };

    let cookie = config.build_cookie("auth_token", "jwt_abc_123");
    assert_eq!(cookie.name(), "auth_token");
    assert_eq!(cookie.value(), "jwt_abc_123");
    assert_eq!(cookie.path(), Some("/auth"));
    assert_eq!(cookie.domain(), Some("example.com"));
    assert_eq!(cookie.http_only(), Some(true));
    assert_eq!(cookie.secure(), Some(true));
}

#[tokio::test]
async fn sets_cookie_in_response() {
    let app = Router::new().route("/set-cookie", get(set_cookie_handler));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/set-cookie")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let set_cookie = response.headers().get(header::SET_COOKIE).unwrap();
    assert!(
        set_cookie
            .to_str()
            .unwrap()
            .contains("session_id=abc123xyz")
    );
}

#[tokio::test]
async fn reads_cookie_from_request() {
    let app = Router::new().route("/get-cookie", get(get_cookie_handler));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/get-cookie")
                .header(
                    header::COOKIE,
                    HeaderValue::from_static("session_id=my_secret_token"),
                )
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(std::str::from_utf8(&bytes).unwrap(), "my_secret_token");
}

#[tokio::test]
async fn signed_cookie_roundtrip() {
    let key = Key::generate();
    let state = AppState { key };

    let app = Router::new()
        .route("/set-signed", get(set_signed_cookie_handler))
        .route("/get-signed", get(get_signed_cookie_handler))
        .with_state(state);

    let set_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/set-signed")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let cookie_header = set_resp.headers().get(header::SET_COOKIE).unwrap().clone();

    let get_resp = app
        .oneshot(
            Request::builder()
                .uri("/get-signed")
                .header(header::COOKIE, cookie_header)
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_resp.status(), StatusCode::OK);
    let bytes = get_resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(std::str::from_utf8(&bytes).unwrap(), "user_42");
}

#[tokio::test]
async fn private_cookie_roundtrip() {
    let key = Key::generate();
    let state = AppState { key };

    let app = Router::new()
        .route("/set-private", get(set_private_cookie_handler))
        .route("/get-private", get(get_private_cookie_handler))
        .with_state(state);

    let set_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/set-private")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let cookie_header = set_resp.headers().get(header::SET_COOKIE).unwrap().clone();

    let get_resp = app
        .oneshot(
            Request::builder()
                .uri("/get-private")
                .header(header::COOKIE, cookie_header)
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_resp.status(), StatusCode::OK);
    let bytes = get_resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(std::str::from_utf8(&bytes).unwrap(), "classified_info");
}
