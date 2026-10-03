//! HTTP router initialization, middleware, and server configuration.

mod config;
pub mod cookie;
pub mod cors;
mod error;
pub mod health;
pub mod http_metrics;
mod middleware;
pub mod request_id;
mod router;
pub mod security_headers;
mod server;
pub mod trace;

pub use axum::Router;
pub use axum::extract::DefaultBodyLimit;
pub use config::HttpConfig;
pub use cookie::{
    Cookie, CookieConfig, CookieJar, Key, PrivateCookieJar, SameSite, SignedCookieJar,
};
pub use cors::{CorsConfig, cors_layer};
pub use error::{AppError, ErrorResponse, log_api_error};
pub use health::{HealthResponse, healthz};
pub use http_metrics::{HttpMetrics, track_http_metrics};
pub use middleware::apply_middleware;
pub use request_id::{MakeRequestUuid, REQUEST_ID_HEADER};
pub use router::{init_router, init_router_with_config};
pub use security_headers::SecurityHeadersConfig;
pub use server::{serve, shutdown_signal};
pub use trace::{trace_context_middleware, trace_layer};
pub use utoipa::{OpenApi, ToSchema};
pub use utoipa_swagger_ui::SwaggerUi;
