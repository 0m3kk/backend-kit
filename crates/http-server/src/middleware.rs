use std::time::Duration;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::middleware::from_fn;
use tower_http::compression::CompressionLayer;
use tower_http::request_id::{PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::timeout::TimeoutLayer;

use crate::config::HttpConfig;
use crate::http_metrics::track_http_metrics;
use crate::request_id::{MakeRequestUuid, REQUEST_ID_HEADER};
use crate::trace::{access_log_middleware, trace_context_middleware, trace_layer};

/// Applies the complete production middleware stack to the router:
/// Request ID -> Trace Context -> Tracing -> Access Log -> HTTP Metrics -> Security Headers -> Timeout -> Compression -> CORS -> Body Limit.
pub fn apply_middleware(mut router: Router, config: &HttpConfig) -> Router {
    router = router.layer(DefaultBodyLimit::max(config.body_limit_bytes));
    router = router.layer(config.cors.build_layer());

    if config.compression {
        router = router.layer(CompressionLayer::new());
    }

    if let Some(timeout) = config.timeout_secs {
        router = router.layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(timeout),
        ));
    }

    if let Some(ref sec) = config.security_headers {
        router = sec.apply(router);
    }

    router
        .layer(from_fn(track_http_metrics))
        .layer(from_fn(access_log_middleware))
        .layer(trace_layer())
        .layer(from_fn(trace_context_middleware))
        .layer(PropagateRequestIdLayer::new(REQUEST_ID_HEADER.clone()))
        .layer(SetRequestIdLayer::new(
            REQUEST_ID_HEADER.clone(),
            MakeRequestUuid,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_full_middleware_stack() {
        let router = Router::new();
        let config = HttpConfig::default();
        let _router = apply_middleware(router, &config);
    }
}
