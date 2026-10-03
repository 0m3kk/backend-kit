use std::fmt;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Standard API error payload returned in JSON format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    /// Human-readable error message.
    #[schema(example = "Resource not found")]
    pub error: String,
    /// Machine-readable error code.
    #[schema(example = "NOT_FOUND")]
    pub code: String,
}

impl ErrorResponse {
    /// Creates a new error response and logs it according to its HTTP status.
    ///
    /// For 5xx server errors, the detailed internal error message is logged server-side,
    /// while the response returned to the client is sanitized ("Internal server error")
    /// to avoid leaking internal implementation, database schema, or infrastructure details.
    pub fn new(
        status: StatusCode,
        code: impl Into<String>,
        error: impl Into<String>,
    ) -> (StatusCode, Json<Self>) {
        let code = code.into();
        let error = error.into();

        log_api_error(status, &code, &error);

        let client_error_msg = if status.is_server_error() {
            "Internal server error".to_string()
        } else {
            error
        };

        (
            status,
            Json(Self {
                error: client_error_msg,
                code,
            }),
        )
    }
}

/// Logs API errors with structured tracing fields (warn for 4xx, error for 5xx).
pub fn log_api_error(status: StatusCode, code: &str, err: &dyn fmt::Display) {
    if status.is_server_error() {
        tracing::error!(
            status = status.as_u16(),
            code = %code,
            error = %err,
            "Internal server error while processing request"
        );
    } else if status.is_client_error() {
        tracing::warn!(
            status = status.as_u16(),
            code = %code,
            error = %err,
            "Client error while processing request"
        );
    }
}

/// Common application errors mapped to HTTP status codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    /// 400 Bad Request
    BadRequest(String),
    /// 401 Unauthorized
    Unauthorized(String),
    /// 403 Forbidden
    Forbidden(String),
    /// 404 Not Found
    NotFound(String),
    /// 409 Conflict
    Conflict(String),
    /// 413 Payload Too Large
    PayloadTooLarge(String),
    /// 408 Request Timeout
    Timeout,
    /// 500 Internal Server Error
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadRequest(msg) => write!(f, "Bad request: {msg}"),
            Self::Unauthorized(msg) => write!(f, "Unauthorized: {msg}"),
            Self::Forbidden(msg) => write!(f, "Forbidden: {msg}"),
            Self::NotFound(msg) => write!(f, "Not found: {msg}"),
            Self::Conflict(msg) => write!(f, "Conflict: {msg}"),
            Self::PayloadTooLarge(msg) => write!(f, "Payload too large: {msg}"),
            Self::Timeout => write!(f, "Request timed out"),
            Self::Internal(msg) => write!(f, "Internal server error: {msg}"),
        }
    }
}

impl std::error::Error for AppError {}

impl AppError {
    /// Returns the corresponding HTTP status code.
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::PayloadTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Timeout => StatusCode::REQUEST_TIMEOUT,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Returns the machine-readable error code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "BAD_REQUEST",
            Self::Unauthorized(_) => "UNAUTHORIZED",
            Self::Forbidden(_) => "FORBIDDEN",
            Self::NotFound(_) => "NOT_FOUND",
            Self::Conflict(_) => "CONFLICT",
            Self::PayloadTooLarge(_) => "PAYLOAD_TOO_LARGE",
            Self::Timeout => "REQUEST_TIMEOUT",
            Self::Internal(_) => "INTERNAL_SERVER_ERROR",
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let code = self.code();
        let error_msg = self.to_string();

        log_api_error(status, code, &error_msg);

        let client_error_msg = if status.is_server_error() {
            "Internal server error".to_string()
        } else {
            error_msg
        };

        let payload = ErrorResponse {
            error: client_error_msg,
            code: code.to_string(),
        };
        (status, Json(payload)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_status_codes_and_codes() {
        assert_eq!(
            AppError::BadRequest("invalid".into()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(AppError::BadRequest("invalid".into()).code(), "BAD_REQUEST");
        assert_eq!(
            AppError::NotFound("item".into()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            AppError::Unauthorized("no auth".into()).status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            AppError::Forbidden("no perm".into()).status_code(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            AppError::Conflict("exists".into()).status_code(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            AppError::PayloadTooLarge("too big".into()).status_code(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert_eq!(AppError::Timeout.status_code(), StatusCode::REQUEST_TIMEOUT);
        assert_eq!(
            AppError::Internal("db fail".into()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
