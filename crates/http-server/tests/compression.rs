#![allow(clippy::unwrap_used)]

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::routing::get;
use http_server::{HttpConfig, apply_middleware, init_router};
use tower::ServiceExt;

async fn large_text_handler() -> String {
    "Hello World! ".repeat(200)
}

#[tokio::test]
async fn compresses_response_with_gzip() {
    let config = HttpConfig {
        compression: true,
        ..Default::default()
    };
    let app = apply_middleware(
        init_router().route("/text", get(large_text_handler)),
        &config,
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/text")
                .header(header::ACCEPT_ENCODING, "gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_ENCODING).unwrap(),
        "gzip"
    );
}

#[tokio::test]
async fn does_not_compress_when_disabled() {
    let config = HttpConfig {
        compression: false,
        ..Default::default()
    };
    let app = apply_middleware(
        init_router().route("/text", get(large_text_handler)),
        &config,
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/text")
                .header(header::ACCEPT_ENCODING, "gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get(header::CONTENT_ENCODING).is_none());
}
