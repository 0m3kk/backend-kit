//! Tracing subscriber and telemetry initialization.

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::config::TelemetryConfig;
use crate::propagation::init_propagator;
use crate::secret::TelemetrySecret;
use crate::tracer::{build_logger_provider, build_meter_provider, build_tracer_provider};

/// Guard that flushes and shuts down OpenTelemetry providers on drop.
pub struct TelemetryGuard {
    tracer_provider: Option<SdkTracerProvider>,
    logger_provider: Option<SdkLoggerProvider>,
    meter_provider: Option<SdkMeterProvider>,
}

impl TelemetryGuard {
    /// Explicitly flushes and shuts down telemetry providers.
    pub fn shutdown(mut self) {
        if let Some(provider) = self.tracer_provider.take() {
            let _ = provider.shutdown();
        }
        if let Some(provider) = self.logger_provider.take() {
            let _ = provider.shutdown();
        }
        if let Some(provider) = self.meter_provider.take() {
            let _ = provider.shutdown();
        }
    }
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(ref provider) = self.tracer_provider {
            let _ = provider.shutdown();
        }
        if let Some(ref provider) = self.logger_provider {
            let _ = provider.shutdown();
        }
        if let Some(ref provider) = self.meter_provider {
            let _ = provider.shutdown();
        }
    }
}

/// Initializes OpenTelemetry tracing, W3C propagation, and console log formatting.
pub fn init_telemetry(
    config: &TelemetryConfig,
    secret: Option<&TelemetrySecret>,
) -> Result<TelemetryGuard, Box<dyn std::error::Error + Send + Sync>> {
    init_propagator();

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    let fmt_layer = tracing_subscriber::fmt::layer();

    if config.enabled {
        let tracer_provider = build_tracer_provider(config, secret)?;
        opentelemetry::global::set_tracer_provider(tracer_provider.clone());

        let tracer = tracer_provider.tracer("tracing-opentelemetry");
        let otel_trace_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        let logger_provider = build_logger_provider(config, secret)?;
        let otel_log_layer = OpenTelemetryTracingBridge::new(&logger_provider);

        let meter_provider = build_meter_provider(config, secret)?;
        opentelemetry::global::set_meter_provider(meter_provider.clone());

        let subscriber = tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .with(otel_trace_layer)
            .with(otel_log_layer);

        subscriber.try_init()?;

        Ok(TelemetryGuard {
            tracer_provider: Some(tracer_provider),
            logger_provider: Some(logger_provider),
            meter_provider: Some(meter_provider),
        })
    } else {
        let subscriber = tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer);

        subscriber.try_init()?;

        Ok(TelemetryGuard {
            tracer_provider: None,
            logger_provider: None,
            meter_provider: None,
        })
    }
}
