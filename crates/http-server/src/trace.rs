use std::time::Duration;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use opentelemetry::propagation::Extractor;
use tower_http::classify::{ServerErrorsAsFailures, SharedClassifier};
use tower_http::trace::TraceLayer;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

struct HeaderExtractor<'a>(&'a axum::http::HeaderMap);

impl Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

/// Extracts an OpenTelemetry context from HTTP headers using registered propagators.
pub fn extract_context(headers: &axum::http::HeaderMap) -> opentelemetry::Context {
    let extractor = HeaderExtractor(headers);
    opentelemetry::global::get_text_map_propagator(|propagator| propagator.extract(&extractor))
}

/// Extracts W3C `traceparent` and `tracestate` headers and attaches them to the current tracing span.
pub async fn trace_context_middleware(req: Request, next: Next) -> Response {
    let parent_cx = extract_context(req.headers());
    let span = Span::current();
    let _ = span.set_parent(parent_cx);

    next.run(req).await
}

/// Redacts sensitive query parameter values (e.g. `token`, `password`, `code`, `secret`, `state`) from a URI.
pub fn sanitize_uri(uri: &axum::http::Uri) -> String {
    if let Some(query) = uri.query() {
        let path = uri.path();
        let sanitized_query: Vec<String> = query
            .split('&')
            .map(|pair| {
                if let Some((k, _)) = pair.split_once('=') {
                    let k_lower = k.to_lowercase();
                    if is_sensitive_query_param(&k_lower) {
                        format!("{k}=[REDACTED]")
                    } else {
                        pair.to_string()
                    }
                } else {
                    pair.to_string()
                }
            })
            .collect();
        format!("{path}?{}", sanitized_query.join("&"))
    } else {
        uri.to_string()
    }
}

fn is_sensitive_query_param(key: &str) -> bool {
    matches!(
        key,
        "code"
            | "token"
            | "access_token"
            | "refresh_token"
            | "id_token"
            | "state"
            | "secret"
            | "client_secret"
            | "password"
            | "pwd"
            | "pass"
            | "key"
            | "api_key"
            | "apikey"
            | "auth"
            | "authorization"
            | "sig"
            | "signature"
            | "recovery_code"
    )
}

/// Single-line HTTP access log middleware.
pub async fn access_log_middleware(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let sanitized_uri = sanitize_uri(&uri);
    let start = std::time::Instant::now();

    let response = next.run(req).await;
    let latency = start.elapsed();
    let status = response.status();
    let status_code = status.as_u16();

    let latency_display = if latency.as_millis() > 0 {
        format!("{}ms", latency.as_millis())
    } else {
        format!("{:.2}ms", latency.as_secs_f64() * 1000.0)
    };

    let reason = status.canonical_reason().unwrap_or("");

    if status.is_server_error() {
        tracing::error!(
            method = %method,
            uri = %sanitized_uri,
            status = status_code,
            latency_ms = latency.as_millis() as u64,
            "{} {} -> {} {} ({})",
            method,
            sanitized_uri,
            status_code,
            reason,
            latency_display
        );
    } else if status.is_client_error() {
        tracing::warn!(
            method = %method,
            uri = %sanitized_uri,
            status = status_code,
            latency_ms = latency.as_millis() as u64,
            "{} {} -> {} {} ({})",
            method,
            sanitized_uri,
            status_code,
            reason,
            latency_display
        );
    } else {
        tracing::info!(
            method = %method,
            uri = %sanitized_uri,
            status = status_code,
            latency_ms = latency.as_millis() as u64,
            "{} {} -> {} {} ({})",
            method,
            sanitized_uri,
            status_code,
            reason,
            latency_display
        );
    }

    response
}

/// Creates a standard [`TraceLayer`] for HTTP OpenTelemetry spans without redundant log records.
#[allow(clippy::type_complexity)]
pub fn trace_layer() -> TraceLayer<
    SharedClassifier<ServerErrorsAsFailures>,
    impl Fn(&axum::http::Request<axum::body::Body>) -> Span + Clone,
    (),
    impl Fn(&Response, Duration, &Span) + Clone,
> {
    TraceLayer::new_for_http()
        .make_span_with(|request: &axum::http::Request<axum::body::Body>| {
            let sanitized_uri = sanitize_uri(request.uri());
            tracing::info_span!(
                "http_request",
                method = %request.method(),
                uri = %sanitized_uri,
                path = %request.uri().path(),
                version = ?request.version(),
                status = tracing::field::Empty,
                latency_ms = tracing::field::Empty,
            )
        })
        .on_request(())
        .on_response(|response: &Response, latency: Duration, span: &Span| {
            span.record("status", response.status().as_u16());
            span.record("latency_ms", latency.as_millis() as u64);
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn creates_trace_layer() {
        let _layer = trace_layer();
    }

    #[test]
    fn sanitizes_sensitive_query_parameters() {
        let uri: axum::http::Uri =
            "/api/v1/auth/oidc/callback?code=super_secret_code&state=secret_state&other=public_val"
                .parse()
                .unwrap();
        let sanitized = sanitize_uri(&uri);
        assert_eq!(
            sanitized,
            "/api/v1/auth/oidc/callback?code=[REDACTED]&state=[REDACTED]&other=public_val"
        );
    }

    #[test]
    fn preserves_uri_without_sensitive_params() {
        let uri: axum::http::Uri = "/api/v1/users?page=1&limit=20".parse().unwrap();
        assert_eq!(sanitize_uri(&uri), "/api/v1/users?page=1&limit=20");
    }
}
