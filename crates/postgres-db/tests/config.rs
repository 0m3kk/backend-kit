use postgres_db::PostgresConfig;

#[test]
fn default_config_creates_valid_instance() {
    let config = PostgresConfig::default();
    assert_eq!(config.host, "localhost");
    assert_eq!(config.port, 5432);
    assert_eq!(config.database, "dev");
    assert_eq!(config.max_connections, 10);
    assert_eq!(config.min_connections, 1);
    assert_eq!(config.connect_timeout_secs, 30);
    assert_eq!(config.idle_timeout_secs, 600);
    assert_eq!(config.max_lifetime_secs, 1800);
}

#[test]
fn serialization_roundtrip() {
    let config = PostgresConfig {
        host: "127.0.0.1".to_string(),
        port: 5433,
        database: "test_db".to_string(),
        max_connections: 20,
        min_connections: 2,
        connect_timeout_secs: 15,
        idle_timeout_secs: 120,
        max_lifetime_secs: 600,
    };

    #[allow(clippy::unwrap_used)]
    let serialized = serde_json::to_string(&config).unwrap();
    #[allow(clippy::unwrap_used)]
    let deserialized: PostgresConfig = serde_json::from_str(&serialized).unwrap();

    assert_eq!(config, deserialized);
}
