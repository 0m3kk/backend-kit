# 📊 telemetry

Production-ready OpenTelemetry tracing, metrics, OTLP export, and W3C context propagation for Rust applications.

---

## ✨ Features

- 🔭 **Unified Observability**: Bridges `tracing` events with OpenTelemetry Traces, Metrics, and Logs via OTLP.
- 📡 **OTLP Exporter**: Exports telemetry data via gRPC tonic to collectors (Grafana Tempo, OpenTelemetry Collector, Jaeger, Prometheus).
- 🔗 **W3C Context Propagation**: Supports standard `traceparent` and `tracestate` header extraction and injection across service boundaries.
- 📈 **Ergonomic Metrics**: High-level wrapper types for [`Counter`], [`Gauge`], [`Histogram`], and RAII [`HistogramTimer`].
- 🛡️ **Secret Redaction**: Configurable redaction of sensitive credentials in telemetry attributes using [`TelemetrySecret`].
- 🧹 **Graceful Flush**: Automatic provider shutdown and buffer flush on drop via [`TelemetryGuard`].

---

## 🚀 Quick Start

### 1. Add Dependency

Add `telemetry` to your `Cargo.toml`:

```toml
[dependencies]
telemetry = "0.1"
```

### 2. Usage Example

```rust
use telemetry::{TelemetryConfig, TelemetrySecret, init_telemetry};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Configure telemetry
    let config = TelemetryConfig::default();
    let secret = TelemetrySecret::default();

    // 2. Initialize telemetry subscriber (holds guard to flush on shutdown)
    let _guard = init_telemetry("my-service", &config, &secret)?;

    info!("Service started with distributed tracing and metrics enabled");

    Ok(())
}
```

---

## 📈 Ergonomic Metrics

```rust
use telemetry::{Counter, Histogram};

let request_counter = Counter::with_scope("api", "http_requests_total");
request_counter.inc_with([("method", "GET"), ("endpoint", "/healthz")]);

let duration_histogram = Histogram::with_scope_and_unit("api", "http_request_duration_seconds", "s");
let _timer = duration_histogram.start_timer([("method", "GET")]);
// Duration is recorded automatically when `_timer` is dropped!
```

---

## 📜 License

Licensed under the [MIT License](LICENSE).
