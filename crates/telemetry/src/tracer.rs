use std::str::FromStr;
use std::time::Duration;

use opentelemetry::KeyValue;
use opentelemetry_otlp::{WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};
use opentelemetry_semantic_conventions::resource::{SERVICE_NAME, SERVICE_VERSION};
use tonic::metadata::{AsciiMetadataKey, AsciiMetadataValue, MetadataMap};

use crate::config::TelemetryConfig;
use crate::secret::TelemetrySecret;

/// Builds an [`SdkTracerProvider`] configured with OTLP gRPC exporter and service metadata.
pub fn build_tracer_provider(
    config: &TelemetryConfig,
    secret: Option<&TelemetrySecret>,
) -> Result<SdkTracerProvider, Box<dyn std::error::Error + Send + Sync>> {
    let mut metadata = MetadataMap::new();
    if let Some(sec) = secret {
        for (key, value) in sec.to_headers() {
            if let Ok(meta_key) = AsciiMetadataKey::from_str(&key)
                && let Ok(meta_val) = AsciiMetadataValue::from_str(&value)
            {
                metadata.insert(meta_key, meta_val);
            }
        }
    }

    let mut exporter_builder = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&config.endpoint)
        .with_timeout(Duration::from_millis(500));

    if !metadata.is_empty() {
        exporter_builder = exporter_builder.with_metadata(metadata);
    }

    let exporter = exporter_builder.build()?;

    let resource = Resource::builder()
        .with_attribute(KeyValue::new(SERVICE_NAME, config.service_name.clone()))
        .with_attribute(KeyValue::new(
            SERVICE_VERSION,
            config.service_version.clone(),
        ))
        .with_attribute(KeyValue::new(
            "deployment.environment.name",
            config.environment.clone(),
        ))
        .build();

    let sampler = if (config.sample_ratio - 1.0).abs() < f64::EPSILON {
        Sampler::AlwaysOn
    } else if config.sample_ratio <= 0.0 {
        Sampler::AlwaysOff
    } else {
        Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(config.sample_ratio)))
    };

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_sampler(sampler)
        .with_resource(resource)
        .build();

    Ok(provider)
}

/// Builds an [`SdkLoggerProvider`] configured with OTLP gRPC exporter and service metadata.
pub fn build_logger_provider(
    config: &TelemetryConfig,
    secret: Option<&TelemetrySecret>,
) -> Result<SdkLoggerProvider, Box<dyn std::error::Error + Send + Sync>> {
    let mut metadata = MetadataMap::new();
    if let Some(sec) = secret {
        for (key, value) in sec.to_headers() {
            if let Ok(meta_key) = AsciiMetadataKey::from_str(&key)
                && let Ok(meta_val) = AsciiMetadataValue::from_str(&value)
            {
                metadata.insert(meta_key, meta_val);
            }
        }
    }

    let mut exporter_builder = opentelemetry_otlp::LogExporter::builder()
        .with_tonic()
        .with_endpoint(&config.endpoint)
        .with_timeout(Duration::from_millis(500));

    if !metadata.is_empty() {
        exporter_builder = exporter_builder.with_metadata(metadata);
    }

    let exporter = exporter_builder.build()?;

    let resource = Resource::builder()
        .with_attribute(KeyValue::new(SERVICE_NAME, config.service_name.clone()))
        .with_attribute(KeyValue::new(
            SERVICE_VERSION,
            config.service_version.clone(),
        ))
        .with_attribute(KeyValue::new(
            "deployment.environment.name",
            config.environment.clone(),
        ))
        .build();

    let provider = SdkLoggerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    Ok(provider)
}

/// Builds an [`SdkMeterProvider`] configured with OTLP gRPC exporter and periodic metric reader.
pub fn build_meter_provider(
    config: &TelemetryConfig,
    secret: Option<&TelemetrySecret>,
) -> Result<opentelemetry_sdk::metrics::SdkMeterProvider, Box<dyn std::error::Error + Send + Sync>>
{
    let mut metadata = MetadataMap::new();
    if let Some(sec) = secret {
        for (key, value) in sec.to_headers() {
            if let Ok(meta_key) = AsciiMetadataKey::from_str(&key)
                && let Ok(meta_val) = AsciiMetadataValue::from_str(&value)
            {
                metadata.insert(meta_key, meta_val);
            }
        }
    }

    let mut exporter_builder = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .with_endpoint(&config.endpoint)
        .with_timeout(Duration::from_millis(500));

    if !metadata.is_empty() {
        exporter_builder = exporter_builder.with_metadata(metadata);
    }

    let exporter = exporter_builder.build()?;

    let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter)
        .with_interval(Duration::from_secs(5))
        .build();

    let resource = Resource::builder()
        .with_attribute(KeyValue::new(SERVICE_NAME, config.service_name.clone()))
        .with_attribute(KeyValue::new(
            SERVICE_VERSION,
            config.service_version.clone(),
        ))
        .with_attribute(KeyValue::new(
            "deployment.environment.name",
            config.environment.clone(),
        ))
        .build();

    let provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder()
        .with_reader(reader)
        .with_resource(resource)
        .build();

    Ok(provider)
}
