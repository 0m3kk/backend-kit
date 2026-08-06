use password_hasher::{Algorithm, AsyncPasswordHasher, PasswordHasher};
use password_hasher_bcrypt::BcryptHasher;

#[test]
fn test_hash_and_verify() {
    let hasher = BcryptHasher::new();
    let password = "test_password_456!";

    let hash = hasher.hash_password(password).unwrap();
    assert_eq!(hash.algorithm(), Algorithm::Bcrypt);
    assert!(hash.as_str().starts_with("$2"));

    assert!(hasher.verify_password(password, &hash).unwrap());
    assert!(!hasher.verify_password("wrong_password", &hash).unwrap());
}

#[test]
fn test_algorithm() {
    let hasher = BcryptHasher::new();
    assert_eq!(hasher.algorithm(), Algorithm::Bcrypt);
}

#[tokio::test]
async fn test_async_hash_and_verify() {
    let hasher = BcryptHasher::new();
    let password = "async_bcrypt_test_789!".to_string();

    let hash = hasher.hash_password_async(password.clone()).await.unwrap();
    assert_eq!(hash.algorithm(), Algorithm::Bcrypt);

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
