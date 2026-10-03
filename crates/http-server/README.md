# 🌐 http-server

Production-ready HTTP router, middleware stack, OpenAPI generator, and server configuration for Rust applications.

---

## ✨ Features

- ⚡ **Axum 0.8 Engine**: Fast, robust HTTP router initialization with graceful shutdown handling (SIGINT/SIGTERM).
- 📊 **OpenTelemetry Metrics**: Built-in request metrics middleware ([`track_http_metrics`]) tracking request count (`http_requests_total`), duration (`http_request_duration_seconds`), in-flight requests (`http_requests_in_flight`), and payload sizes.
- 🔗 **Distributed Tracing**: Automatic W3C `traceparent` context extraction middleware ([`trace_context_middleware`]) linking incoming upstream traces.
- 🆔 **Request ID Tracking**: Automatic UUID v4 generation and header propagation via `x-request-id`.
- 🛡️ **Security Headers**: Production security headers (HSTS, CSP, X-Content-Type-Options, Frame-Options, etc.) via [`SecurityHeadersConfig`].
- ⏱️ **Timeouts & Limits**: Per-request timeout and body size limit protections ([`DefaultBodyLimit`]).
- 🗜️ **Response Compression**: Multi-algorithm compression support (`gzip`, `brotli`, `zstd`).
- 🌐 **CORS Management**: Fine-grained CORS configuration via [`CorsConfig`].
- 🍪 **Cookie Support**: Plain, signed, and private encrypted cookie jars via [`CookieConfig`].
- 📑 **OpenAPI / Swagger UI**: Built-in integration with `utoipa` and `utoipa-swagger-ui`.

---

## 🚀 Quick Start

### 1. Add Dependency

Add `http-server` to your `Cargo.toml`:

```toml
[dependencies]
http-server = "0.1"
```

### 2. Usage Example

```rust
use http_server::{HttpConfig, init_router_with_config, serve};
use axum::routing::get;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize configuration (from defaults or environment)
    let config = HttpConfig::default();

    // 2. Initialize router with health check (/healthz) and full middleware stack
    let app = init_router_with_config(&config)
        .route("/api/v1/hello", get(|| async { "Hello, world!" }));

    // 3. Start server with graceful shutdown
    serve(&config, app).await?;

    Ok(())
}
```

---

## 🧩 Middleware Stack

The production middleware stack ([`apply_middleware`]) executes in the following order:

1. **W3C Trace Context**: Extracts upstream trace information from headers into tracing span.
2. **Request ID**: Attaches unique UUID v4 identifier.
3. **HTTP Metrics**: Observes latency, in-flight gauges, and status codes.
4. **Structured Logging**: Emits access logs with sanitized URIs.
5. **CORS**: Handles preflight and origin verification.
6. **Compression**: Compresses response payloads (brotli, gzip, zstd).
7. **Timeout**: Limits request processing execution time.
8. **Security Headers**: Sets modern defense-in-depth HTTP headers.
9. **Body Limit**: Enforces maximum allowable request body payload.

---

## 🧪 Testing

Test endpoints using `tower::ServiceExt` without starting a network listener:

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_server::{HttpConfig, init_router_with_config};
use tower::ServiceExt;

#[tokio::test]
async fn test_health_check() {
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
```

---

## 📜 License

Licensed under the [MIT License](LICENSE).
