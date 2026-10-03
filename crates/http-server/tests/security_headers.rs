#![allow(clippy::unwrap_used)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_server::security_headers::{
    CONTENT_SECURITY_POLICY, REFERRER_POLICY, STRICT_TRANSPORT_SECURITY, X_CONTENT_TYPE_OPTIONS,
    X_FRAME_OPTIONS, X_XSS_PROTECTION,
};
use http_server::{HttpConfig, SecurityHeadersConfig, init_router_with_config};
use tower::ServiceExt;

#[tokio::test]
async fn attaches_default_security_headers() {
    let config = HttpConfig::default();
    let app = init_router_with_config(&config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(X_CONTENT_TYPE_OPTIONS.clone())
            .unwrap(),
        "nosniff"
    );
    assert_eq!(
        response.headers().get(X_FRAME_OPTIONS.clone()).unwrap(),
        "DENY"
    );
    assert_eq!(
        response.headers().get(X_XSS_PROTECTION.clone()).unwrap(),
        "0"
    );
    assert_eq!(
        response
            .headers()
            .get(STRICT_TRANSPORT_SECURITY.clone())
            .unwrap(),
        "max-age=31536000; includeSubDomains"
    );
    assert_eq!(
        response.headers().get(REFERRER_POLICY.clone()).unwrap(),
        "strict-origin-when-cross-origin"
    );
}

#[tokio::test]
async fn custom_security_headers_configuration() {
    let config = HttpConfig {
        security_headers: Some(SecurityHeadersConfig {
            content_type_options: Some("nosniff".to_string()),
            frame_options: Some("SAMEORIGIN".to_string()),
            xss_protection: None,
            hsts: None,
            content_security_policy: Some("default-src 'self'".to_string()),
            referrer_policy: Some("no-referrer".to_string()),
        }),
        ..Default::default()
    };
    let app = init_router_with_config(&config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(X_CONTENT_TYPE_OPTIONS.clone())
            .unwrap(),
        "nosniff"
    );
    assert_eq!(
        response.headers().get(X_FRAME_OPTIONS.clone()).unwrap(),
        "SAMEORIGIN"
    );
    assert!(response.headers().get(X_XSS_PROTECTION.clone()).is_none());
    assert!(
        response
            .headers()
            .get(STRICT_TRANSPORT_SECURITY.clone())
            .is_none()
    );
    assert_eq!(
        response
            .headers()
            .get(CONTENT_SECURITY_POLICY.clone())
            .unwrap(),
        "default-src 'self'"
    );
    assert_eq!(
        response.headers().get(REFERRER_POLICY.clone()).unwrap(),
        "no-referrer"
    );
}

#[tokio::test]
async fn disables_security_headers_when_none() {
    let config = HttpConfig {
        security_headers: None,
        ..Default::default()
    };
    let app = init_router_with_config(&config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get(X_CONTENT_TYPE_OPTIONS.clone())
            .is_none()
    );
    assert!(response.headers().get(X_FRAME_OPTIONS.clone()).is_none());
}
