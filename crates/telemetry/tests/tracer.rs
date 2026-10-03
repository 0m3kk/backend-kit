use telemetry::{TelemetryConfig, build_tracer_provider};

#[tokio::test]
async fn builds_tracer_provider_with_defaults() {
    let config = TelemetryConfig::default();
    #[allow(clippy::unwrap_used)]
    let provider = build_tracer_provider(&config, None).unwrap();
    std::mem::forget(provider);
}

#[tokio::test]
async fn builds_tracer_provider_with_custom_sampling() {
    let config = TelemetryConfig {
        sample_ratio: 0.5,
        ..Default::default()
    };
    #[allow(clippy::unwrap_used)]
    let provider = build_tracer_provider(&config, None).unwrap();
    std::mem::forget(provider);
}
