#![allow(clippy::unwrap_used)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_server::{HttpConfig, REQUEST_ID_HEADER, init_router_with_config};
use tower::ServiceExt;

#[tokio::test]
async fn assigns_request_id_when_not_provided() {
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
    let request_id = response.headers().get(REQUEST_ID_HEADER.clone()).unwrap();
    assert!(!request_id.is_empty());
}

#[tokio::test]
async fn propagates_custom_request_id() {
    let config = HttpConfig::default();
    let app = init_router_with_config(&config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header(REQUEST_ID_HEADER.clone(), "custom-req-id-12345")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let request_id = response.headers().get(REQUEST_ID_HEADER.clone()).unwrap();
    assert_eq!(request_id, "custom-req-id-12345");
}
