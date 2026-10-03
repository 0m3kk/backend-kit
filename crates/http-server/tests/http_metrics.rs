#![allow(clippy::unwrap_used)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_server::{HttpConfig, init_router_with_config};
use tower::ServiceExt;

#[tokio::test]
async fn track_http_metrics_processes_requests_successfully() {
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
}
