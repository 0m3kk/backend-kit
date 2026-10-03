#![allow(clippy::unwrap_used)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_server::cors::CorsConfig;
use http_server::{HttpConfig, init_router_with_config};
use tower::ServiceExt;

#[tokio::test]
async fn cors_preflight_request_wildcard() {
    let config = HttpConfig::default();
    let app = init_router_with_config(&config);

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/healthz")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&header::HeaderValue::from_static("*"))
    );
}

#[tokio::test]
async fn cors_with_specific_allowed_origin() {
    let config = HttpConfig {
        cors: CorsConfig {
            allow_origins: vec!["https://app.example.com".to_string()],
            ..Default::default()
        },
        ..Default::default()
    };
    let app = init_router_with_config(&config);

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/healthz")
                .header(header::ORIGIN, "https://app.example.com")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&header::HeaderValue::from_static("https://app.example.com"))
    );
}

#[tokio::test]
async fn cors_exposes_headers() {
    let config = HttpConfig {
        cors: CorsConfig {
            expose_headers: vec!["X-Custom-Header".to_string()],
            ..Default::default()
        },
        ..Default::default()
    };
    let app = init_router_with_config(&config);

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/healthz")
                .header(header::ORIGIN, "http://localhost:3000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::ACCESS_CONTROL_EXPOSE_HEADERS),
        Some(&header::HeaderValue::from_static("x-custom-header"))
    );
}
