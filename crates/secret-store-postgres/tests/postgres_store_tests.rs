#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use secret_store::{
    CipherAlgorithm, KeyProvider, ListSecretOptions, SecretPath, SecretStore, SecretValue,
    SetSecretOptions, StaticKeyProvider,
};
use secret_store_postgres::PostgresSecretStore;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

fn create_key_provider() -> Arc<dyn KeyProvider> {
    let mut provider = StaticKeyProvider::new();
    provider
        .add_key("k1", vec![11u8; 32])
        .expect("Failed to add key k1");
    provider
        .add_key("k2", vec![22u8; 32])
        .expect("Failed to add key k2");
    Arc::new(provider)
}

#[tokio::test]
async fn test_postgres_secret_store_lazy_connect() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://postgres:postgres@localhost:5432/test_db")
        .expect("Lazy connect failed");

    let provider = create_key_provider();
    let store = PostgresSecretStore::new(pool, provider, "k1", CipherAlgorithm::Aes256Gcm);
    let path = SecretPath::new("test/lazy").unwrap();
    assert!(store.get(&path).await.is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_postgres_secret_get_set_delete(pool: PgPool) {
    let provider = create_key_provider();
    let store = PostgresSecretStore::new(pool, provider, "k1", CipherAlgorithm::Aes256Gcm);

    let path = SecretPath::new("prod/db/app_password").unwrap();
    let value = SecretValue::from("super_secret_db_pass");

    assert!(store.get(&path).await.unwrap().is_none());

    let entry = store
        .set(
            path.clone(),
            value.clone(),
            SetSecretOptions::new().with_tag("env", "prod"),
        )
        .await
        .unwrap();

    assert_eq!(entry.version, 1);
    assert_eq!(entry.tags.get("env"), Some(&"prod".to_string()));

    let fetched = store.get(&path).await.unwrap().unwrap();
    assert_eq!(fetched.version, 1);
    assert_eq!(fetched.value.as_str().unwrap(), "super_secret_db_pass");

    // Version update
    let value_v2 = SecretValue::from("new_secret_db_pass");
    let entry_v2 = store
        .set(path.clone(), value_v2, SetSecretOptions::new())
        .await
        .unwrap();

    assert_eq!(entry_v2.version, 2);

    let fetched_v1 = store.get_version(&path, 1).await.unwrap().unwrap();
    assert_eq!(fetched_v1.value.as_str().unwrap(), "super_secret_db_pass");

    let fetched_v2 = store.get_version(&path, 2).await.unwrap().unwrap();
    assert_eq!(fetched_v2.value.as_str().unwrap(), "new_secret_db_pass");

    // Delete
    assert!(store.delete(&path).await.unwrap());
    assert!(store.get(&path).await.unwrap().is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_postgres_secret_list_and_rotate(pool: PgPool) {
    let provider = create_key_provider();
    let store = PostgresSecretStore::new(pool, provider, "k1", CipherAlgorithm::Aes256Gcm);

    let path1 = SecretPath::new("prod/services/stripe").unwrap();
    let path2 = SecretPath::new("prod/services/aws").unwrap();

    store
        .set(
            path1.clone(),
            SecretValue::from("stripe_key"),
            SetSecretOptions::new().with_tag("service", "stripe"),
        )
        .await
        .unwrap();

    store
        .set(
            path2.clone(),
            SecretValue::from("aws_key"),
            SetSecretOptions::new().with_tag("service", "aws"),
        )
        .await
        .unwrap();

    let list_res = store
        .list(ListSecretOptions::new().with_prefix(SecretPath::new("prod/services").unwrap()))
        .await
        .unwrap();
    assert_eq!(list_res.len(), 2);

    // Rotate master key k1 -> k2
    let count = store.rotate_key("k1", "k2").await.unwrap();
    assert_eq!(count, 2);

    let s1 = store.get(&path1).await.unwrap().unwrap();
    assert_eq!(s1.value.as_str().unwrap(), "stripe_key");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_postgres_secret_expiration(pool: PgPool) {
    let provider = create_key_provider();
    let store = PostgresSecretStore::new(pool, provider, "k1", CipherAlgorithm::Aes256Gcm);

    let path = SecretPath::new("temp/token").unwrap();
    store
        .set(
            path.clone(),
            SecretValue::from("temp_val"),
            SetSecretOptions::new().with_ttl(Duration::from_millis(50)),
        )
        .await
        .unwrap();

    assert!(store.get(&path).await.unwrap().is_some());
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(store.get(&path).await.unwrap().is_none());

    let cleaned = store.clean_expired(None).await.unwrap();
    assert_eq!(cleaned, 1);
}
