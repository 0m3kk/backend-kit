#![allow(clippy::unwrap_used)]

use postgres_db::{PostgresConfig, PostgresSecret, connect_options, init_pool};

#[test]
fn creates_connect_options_from_config_and_secret() {
    let config = PostgresConfig::default();
    let secret = PostgresSecret::default();
    let options = connect_options(&config, &secret);
    assert_eq!(options.get_host(), "localhost");
    assert_eq!(options.get_port(), 5432);
    assert_eq!(options.get_database(), Some("dev"));
    assert_eq!(options.get_username(), "postgres");
}

#[tokio::test]
async fn connects_pool_with_default_config() {
    let config = PostgresConfig::default();
    let secret = PostgresSecret::default();
    let pool = init_pool(&config, &secret).await.unwrap();

    let row: (i32,) = sqlx::query_as("SELECT 1").fetch_one(&pool).await.unwrap();
    assert_eq!(row.0, 1);
}

#[tokio::test]
async fn config_method_inits_pool() {
    let config = PostgresConfig::default();
    let secret = PostgresSecret::default();
    let pool = config.init_pool(&secret).await.unwrap();

    let row: (i32,) = sqlx::query_as("SELECT 1").fetch_one(&pool).await.unwrap();
    assert_eq!(row.0, 1);
}
