use postgres_db::PostgresSecret;

#[test]
fn default_secret_creates_valid_instance() {
    let secret = PostgresSecret::default();
    assert_eq!(secret.username, "postgres");
    assert_eq!(secret.password, "password");
}

#[test]
fn serialization_roundtrip() {
    let secret = PostgresSecret {
        username: "admin".to_string(),
        password: "secret_value".to_string(),
    };

    #[allow(clippy::unwrap_used)]
    let serialized = serde_json::to_string(&secret).unwrap();
    #[allow(clippy::unwrap_used)]
    let deserialized: PostgresSecret = serde_json::from_str(&serialized).unwrap();

    assert_eq!(secret, deserialized);
}
