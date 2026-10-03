//! HTTP metrics recording middleware.

use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::http::header::CONTENT_LENGTH;
use axum::middleware::Next;
use axum::response::Response;
use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry::metrics::{Counter, Histogram, UpDownCounter};

/// Global HTTP server metrics holder.
#[derive(Clone)]
pub struct HttpMetrics {
    requests_total: Counter<u64>,
    request_duration: Histogram<f64>,
    requests_in_flight: UpDownCounter<i64>,
    request_body_bytes: Histogram<f64>,
    response_body_bytes: Histogram<f64>,
}

impl Default for HttpMetrics {
    fn default() -> Self {
        let meter = global::meter("http-server");
        Self {
            requests_total: meter.u64_counter("http_requests_total").build(),
            request_duration: meter
                .f64_histogram("http_request_duration_seconds")
                .with_unit("s")
                .build(),
            requests_in_flight: meter.i64_up_down_counter("http_requests_in_flight").build(),
            request_body_bytes: meter
                .f64_histogram("http_request_body_bytes")
                .with_unit("By")
                .build(),
            response_body_bytes: meter
                .f64_histogram("http_response_body_bytes")
                .with_unit("By")
                .build(),
        }
    }
}

/// Middleware that records HTTP request counts, response durations, in-flight counts, and payload sizes.
pub async fn track_http_metrics(req: Request, next: Next) -> Response {
    let metrics = HttpMetrics::default();

    let method = req.method().to_string();
    let uri_path = req.uri().path().to_string();

    let req_size: f64 = req
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    metrics
        .requests_in_flight
        .add(1, &[KeyValue::new("method", method.clone())]);

    let start = Instant::now();
    let response = next.run(req).await;
    let duration = start.elapsed().as_secs_f64();

    // Use MatchedPath if available (e.g. /users/:id) to prevent high cardinality, fallback to uri path
    let path = response
        .extensions()
        .get::<MatchedPath>()
        .map(|mp| mp.as_str().to_string())
        .unwrap_or(uri_path);

    let status = response.status().as_u16().to_string();

    let res_size: f64 = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    metrics
        .requests_in_flight
        .add(-1, &[KeyValue::new("method", method.clone())]);

    metrics.requests_total.add(
        1,
        &[
            KeyValue::new("method", method.clone()),
            KeyValue::new("path", path.clone()),
            KeyValue::new("status", status.clone()),
        ],
    );

    metrics.request_duration.record(
        duration,
        &[
            KeyValue::new("method", method.clone()),
            KeyValue::new("path", path.clone()),
            KeyValue::new("status", status.clone()),
        ],
    );

    metrics.request_body_bytes.record(
        req_size,
        &[
            KeyValue::new("method", method.clone()),
            KeyValue::new("path", path.clone()),
        ],
    );

    metrics.response_body_bytes.record(
        res_size,
        &[
            KeyValue::new("method", method),
            KeyValue::new("path", path),
            KeyValue::new("status", status),
        ],
    );

    response
}
