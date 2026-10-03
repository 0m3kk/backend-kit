use axum::Json;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Health check response payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    /// Status indicator (e.g. `"ok"`).
    #[schema(example = "ok")]
    pub status: String,
}

/// Handler for health check probe (`/healthz`).
#[utoipa::path(
    get,
    path = "/healthz",
    tag = "Health",
    responses(
        (status = 200, description = "Server is healthy", body = HealthResponse)
    )
)]
pub async fn healthz() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn healthz_returns_ok_status() {
        let response = healthz().await.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }
}
