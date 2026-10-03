#![allow(clippy::unwrap_used)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_server::{HttpConfig, init_router_with_config};
use tower::ServiceExt;

#[tokio::test]
async fn handles_incoming_traceparent_header() {
    let config = HttpConfig::default();
    let app = init_router_with_config(&config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header(
                    "traceparent",
                    "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
