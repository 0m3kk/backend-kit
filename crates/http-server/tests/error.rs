#![allow(clippy::unwrap_used)]

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use http_body_util::BodyExt;
use http_server::{AppError, ErrorResponse};
use tower::ServiceExt;

async fn handler_bad_request() -> Result<&'static str, AppError> {
    Err(AppError::BadRequest("invalid input".into()))
}

async fn handler_not_found() -> Result<&'static str, AppError> {
    Err(AppError::NotFound("item 123".into()))
}

async fn handler_unauthorized() -> Result<&'static str, AppError> {
    Err(AppError::Unauthorized("missing token".into()))
}

async fn handler_forbidden() -> Result<&'static str, AppError> {
    Err(AppError::Forbidden("insufficient permissions".into()))
}

#[tokio::test]
async fn returns_json_error_for_bad_request() {
    let app = Router::new().route("/bad", get(handler_bad_request));

    let response = app
        .oneshot(Request::builder().uri("/bad").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let err: ErrorResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err.error, "Bad request: invalid input");
    assert_eq!(err.code, "BAD_REQUEST");
}

#[tokio::test]
async fn returns_json_error_for_not_found() {
    let app = Router::new().route("/missing", get(handler_not_found));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/missing")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let err: ErrorResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err.error, "Not found: item 123");
    assert_eq!(err.code, "NOT_FOUND");
}

#[tokio::test]
async fn returns_json_error_for_unauthorized() {
    let app = Router::new().route("/auth", get(handler_unauthorized));

    let response = app
        .oneshot(Request::builder().uri("/auth").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn returns_json_error_for_forbidden() {
    let app = Router::new().route("/forbidden", get(handler_forbidden));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/forbidden")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn sanitizes_500_internal_server_error_for_client() {
    async fn handler_internal() -> Result<&'static str, AppError> {
        Err(AppError::Internal(
            "sensitive database connection failed with credentials leaked".into(),
        ))
    }

    let app = Router::new().route("/error500", get(handler_internal));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/error500")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let err: ErrorResponse = serde_json::from_slice(&bytes).unwrap();
    // Detail must be hidden from client
    assert_eq!(err.error, "Internal server error");
    assert_eq!(err.code, "INTERNAL_SERVER_ERROR");
}

#[test]
fn error_response_new_sanitizes_500_while_preserving_4xx() {
    // 500 is sanitized
    let (status, json) = ErrorResponse::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "STORE_ERROR",
        "raw postgres connection timed out: host=127.0.0.1 db=secret",
    );
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(json.0.error, "Internal server error");
    assert_eq!(json.0.code, "STORE_ERROR");

    // 400 preserves exact error message for user
    let (status, json) = ErrorResponse::new(
        StatusCode::BAD_REQUEST,
        "VALIDATION_ERROR",
        "Username must be at least 3 characters",
    );
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json.0.error, "Username must be at least 3 characters");
    assert_eq!(json.0.code, "VALIDATION_ERROR");
}
