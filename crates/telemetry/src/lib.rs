//! OpenTelemetry tracing, metrics, OTLP export, and W3C context propagation.

pub mod config;
pub mod metrics;
pub mod propagation;
pub mod secret;
mod subscriber;
pub mod tracer;

pub use config::TelemetryConfig;
pub use metrics::{
    Counter, Gauge, Histogram, HistogramTimer, IntoKeyValue, ValueGauge, to_key_values,
};
pub use propagation::{extract_context, init_propagator, inject_context};
pub use secret::TelemetrySecret;
pub use subscriber::{TelemetryGuard, init_telemetry};
pub use tracer::{build_logger_provider, build_meter_provider, build_tracer_provider};
