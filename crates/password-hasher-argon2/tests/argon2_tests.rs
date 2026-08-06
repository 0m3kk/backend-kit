use password_hasher::{Algorithm, AsyncPasswordHasher, PasswordHasher};
use password_hasher_argon2::Argon2Hasher;

#[test]
fn test_hash_and_verify() {
    let hasher = Argon2Hasher::new();
    let password = "test_password_123!";

    let hash = hasher.hash_password(password).unwrap();
    assert_eq!(hash.algorithm(), Algorithm::Argon2id);
    assert!(hash.as_str().starts_with("$argon2id$"));

    assert!(hasher.verify_password(password, &hash).unwrap());
    assert!(!hasher.verify_password("wrong_password", &hash).unwrap());
}

#[test]
fn test_algorithm() {
    let hasher = Argon2Hasher::new();
    assert_eq!(hasher.algorithm(), Algorithm::Argon2id);
}

#[tokio::test]
async fn test_async_hash_and_verify() {
    let hasher = Argon2Hasher::new();
    let password = "async_test_password_789!".to_string();

    let hash = hasher.hash_password_async(password.clone()).await.unwrap();
    assert_eq!(hash.algorithm(), Algorithm::Argon2id);

    assert!(
        hasher
            .verify_password_async(password.clone(), hash.clone())
            .await
            .unwrap()
    );

    assert!(
        !hasher
            .verify_password_async("wrong_pass".to_string(), hash)
            .await
            .unwrap()
    );
}
