#![allow(clippy::unwrap_used)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use http_server::{HttpConfig, apply_middleware, init_router};
use tower::ServiceExt;

async fn slow_handler() -> &'static str {
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    "done"
}

async fn fast_handler() -> &'static str {
    "quick"
}

#[tokio::test]
async fn fast_request_completes_within_timeout() {
    let config = HttpConfig {
        timeout_secs: Some(1),
        ..Default::default()
    };
    let app = apply_middleware(init_router().route("/fast", get(fast_handler)), &config);

    let response = app
        .oneshot(Request::builder().uri("/fast").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn slow_request_triggers_timeout() {
    let config = HttpConfig {
        timeout_secs: Some(0), // 0 seconds timeout -> immediate timeout
        ..Default::default()
    };
    let app = apply_middleware(init_router().route("/slow", get(slow_handler)), &config);

    let response = app
        .oneshot(Request::builder().uri("/slow").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
}
