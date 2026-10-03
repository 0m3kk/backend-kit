#![allow(clippy::unwrap_used)]

use axum::body::Bytes;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use http_server::{HttpConfig, apply_middleware, init_router};
use tower::ServiceExt;

async fn echo_handler(body: Bytes) -> StatusCode {
    let _ = body;
    StatusCode::OK
}

#[tokio::test]
async fn body_within_limit_succeeds() {
    let config = HttpConfig {
        body_limit_bytes: 100,
        ..Default::default()
    };
    let app = apply_middleware(init_router().route("/echo", post(echo_handler)), &config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/echo")
                .body(axum::body::Body::from("hello"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn body_exceeding_limit_is_rejected() {
    let config = HttpConfig {
        body_limit_bytes: 5,
        ..Default::default()
    };
    let app = apply_middleware(init_router().route("/echo", post(echo_handler)), &config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/echo")
                .body(axum::body::Body::from(
                    "this is a very long string exceeding limit",
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
