use password_hasher::noop::NoopHasher;
use password_hasher::{Algorithm, AsyncPasswordHasher, PasswordHasher};

#[test]
fn test_hash_and_verify() {
    let hasher = NoopHasher::new();
    let password = "plaintext_password";

    let hash = hasher.hash_password(password).unwrap();
    assert_eq!(hash.algorithm(), Algorithm::Noop);
    assert_eq!(hash.as_str(), "$noop$plaintext_password");

    assert!(hasher.verify_password(password, &hash).unwrap());
    assert!(!hasher.verify_password("wrong_password", &hash).unwrap());
}

#[test]
fn test_algorithm() {
    let hasher = NoopHasher::new();
    assert_eq!(hasher.algorithm(), Algorithm::Noop);
}

#[tokio::test]
async fn test_async_hash_and_verify() {
    let hasher = NoopHasher::new();
    let password = "async_plaintext".to_string();

    let hash = hasher.hash_password_async(password.clone()).await.unwrap();
    assert_eq!(hash.algorithm(), Algorithm::Noop);

    assert!(
        hasher
            .verify_password_async(password.clone(), hash.clone())
            .await
            .unwrap()
    );
    assert!(
        !hasher
            .verify_password_async("wrong".to_string(), hash)
            .await
            .unwrap()
    );
}
