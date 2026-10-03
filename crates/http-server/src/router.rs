use axum::Router;
use axum::routing::get;

use crate::config::HttpConfig;
use crate::health;
use crate::middleware::apply_middleware;

/// Initializes the base HTTP router with the health check endpoint (`/healthz`).
pub fn init_router() -> Router {
    Router::new().route("/healthz", get(health::healthz))
}

/// Initializes the HTTP router with the health check endpoint and configured middleware.
pub fn init_router_with_config(config: &HttpConfig) -> Router {
    apply_middleware(init_router(), config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_router() {
        let _router = init_router();
    }

    #[test]
    fn initializes_router_with_config() {
        let config = HttpConfig::default();
        let _router = init_router_with_config(&config);
    }
}
