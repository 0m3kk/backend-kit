use serde::{Deserialize, Serialize};

/// OpenTelemetry and logging configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// Name of the service.
    #[serde(default = "default_service_name")]
    pub service_name: String,

    /// Version of the service.
    #[serde(default = "default_service_version")]
    pub service_version: String,

    /// Deployment environment (e.g. `development`, `production`).
    #[serde(default = "default_environment")]
    pub environment: String,

    /// OTLP gRPC/HTTP endpoint (e.g. `http://localhost:4317`).
    #[serde(default = "default_endpoint")]
    pub endpoint: String,

    /// Whether OpenTelemetry export is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Sampling ratio for traces (0.0 to 1.0).
    #[serde(default = "default_sample_ratio")]
    pub sample_ratio: f64,

    /// Log filtering directives (e.g. `info`, `debug,http_server=trace`).
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_service_name() -> String {
    "backend-api".to_string()
}

fn default_service_version() -> String {
    "0.1.0".to_string()
}

fn default_environment() -> String {
    "development".to_string()
}

fn default_endpoint() -> String {
    "http://localhost:4317".to_string()
}

const fn default_enabled() -> bool {
    true
}

const fn default_sample_ratio() -> f64 {
    1.0
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            service_name: default_service_name(),
            service_version: default_service_version(),
            environment: default_environment(),
            endpoint: default_endpoint(),
            enabled: default_enabled(),
            sample_ratio: default_sample_ratio(),
            log_level: default_log_level(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
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
    fn deserializes_from_json() {
        let json = r#"{
            "service_name": "order-service",
            "service_version": "1.2.0",
            "environment": "staging",
            "endpoint": "http://otel-collector:4317",
            "enabled": false,
            "sample_ratio": 0.5,
            "log_level": "debug"
        }"#;

        #[allow(clippy::unwrap_used)]
        let config: TelemetryConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.service_name, "order-service");
        assert_eq!(config.service_version, "1.2.0");
        assert_eq!(config.environment, "staging");
        assert_eq!(config.endpoint, "http://otel-collector:4317");
        assert!(!config.enabled);
        assert!((config.sample_ratio - 0.5).abs() < f64::EPSILON);
        assert_eq!(config.log_level, "debug");
    }
}
