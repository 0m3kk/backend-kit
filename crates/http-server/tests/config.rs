use http_server::HttpConfig;
use http_server::cookie::{CookieConfig, SameSite};
use http_server::cors::CorsConfig;
use http_server::security_headers::SecurityHeadersConfig;

#[test]
fn default_config_creates_valid_instance() {
    let config = HttpConfig::default();
    assert_eq!(config.host, "0.0.0.0");
    assert_eq!(config.port, 8080);
    assert_eq!(config.body_limit_bytes, 2 * 1024 * 1024);
    assert_eq!(config.timeout_secs, Some(30));
    assert!(config.compression);
    assert_eq!(
        config.security_headers,
        Some(SecurityHeadersConfig::default())
    );
    assert_eq!(config.cors, CorsConfig::default());
    assert_eq!(config.cookie, CookieConfig::default());
}

#[test]
fn serialization_roundtrip() {
    let config = HttpConfig {
        host: "127.0.0.1".to_string(),
        port: 3000,
        body_limit_bytes: 1048576,
        timeout_secs: Some(15),
        compression: false,
        security_headers: Some(SecurityHeadersConfig {
            content_type_options: Some("nosniff".to_string()),
            frame_options: Some("SAMEORIGIN".to_string()),
            xss_protection: None,
            hsts: Some("max-age=63072000".to_string()),
            content_security_policy: Some("default-src 'self'".to_string()),
            referrer_policy: Some("no-referrer".to_string()),
        }),
        cors: CorsConfig {
            allow_origins: vec!["https://example.com".to_string()],
            allow_methods: vec!["GET".to_string(), "POST".to_string()],
            allow_headers: vec!["Authorization".to_string()],
            expose_headers: vec!["X-Total-Count".to_string()],
            allow_credentials: true,
            max_age_secs: Some(7200),
        },
        cookie: CookieConfig {
            domain: Some("example.com".to_string()),
            path: "/api".to_string(),
            secure: true,
            http_only: true,
            same_site: SameSite::Strict,
            max_age_secs: Some(86400),
        },
    };

    #[allow(clippy::unwrap_used)]
    let serialized = serde_json::to_string(&config).unwrap();
    #[allow(clippy::unwrap_used)]
    let deserialized: HttpConfig = serde_json::from_str(&serialized).unwrap();

    assert_eq!(config, deserialized);
}
