use telemetry::TelemetryConfig;

#[test]
fn default_config_creates_valid_instance() {
    let config = TelemetryConfig::default();
    assert_eq!(config.service_name, "backend-api");
    assert_eq!(config.service_version, "0.1.0");
    assert_eq!(config.environment, "development");
    assert_eq!(config.endpoint, "http://localhost:4317");
    assert!(config.enabled);
    assert!((config.sample_ratio - 1.0).abs() < f64::EPSILON);
    assert_eq!(config.log_level, "info");
}

#[test]
fn serialization_roundtrip() {
    let config = TelemetryConfig {
        service_name: "custom-api".to_string(),
        service_version: "2.0.0".to_string(),
        environment: "production".to_string(),
        endpoint: "http://otel.internal:4317".to_string(),
        enabled: true,
        sample_ratio: 0.1,
        log_level: "warn".to_string(),
    };

    #[allow(clippy::unwrap_used)]
    let serialized = serde_json::to_string(&config).unwrap();
    #[allow(clippy::unwrap_used)]
    let deserialized: TelemetryConfig = serde_json::from_str(&serialized).unwrap();

    assert_eq!(config, deserialized);
}
