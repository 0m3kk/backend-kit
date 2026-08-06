use std::sync::Arc;

use password_hasher::{
    Algorithm, Argon2Hasher, AsyncPasswordHasher, BcryptHasher, NoopHasher, PasswordError,
    PasswordHash, PasswordHasher, PasswordHasherManager,
};

#[cfg(feature = "pbkdf2")]
use password_hasher::Pbkdf2Hasher;

#[test]
fn test_algorithm_display_and_parse() {
    assert_eq!(Algorithm::Argon2id.to_string(), "argon2id");
    assert_eq!(Algorithm::Bcrypt.to_string(), "bcrypt");
    assert_eq!(Algorithm::Pbkdf2Sha256.to_string(), "pbkdf2-sha256");
    assert_eq!(Algorithm::Noop.to_string(), "noop");

    assert_eq!(
        "argon2id".parse::<Algorithm>().unwrap(),
        Algorithm::Argon2id
    );
    assert_eq!("bcrypt".parse::<Algorithm>().unwrap(), Algorithm::Bcrypt);
    assert_eq!("2b".parse::<Algorithm>().unwrap(), Algorithm::Bcrypt);
    assert_eq!(
        "pbkdf2-sha256".parse::<Algorithm>().unwrap(),
        Algorithm::Pbkdf2Sha256
    );
    assert_eq!("noop".parse::<Algorithm>().unwrap(), Algorithm::Noop);

    assert!("unknown_alg".parse::<Algorithm>().is_err());
}

#[test]
fn test_password_hash_auto_detection() {
    let argon_hash = PasswordHash::parse(
        "$argon2id$v=19$m=65536,t=3,p=4$c29tZXNhbHQ$RGF0YWJhc2VTZWNyZXRLZXlIYXNoVmFsdWU",
    )
    .unwrap();
    assert_eq!(argon_hash.algorithm(), Algorithm::Argon2id);

    let bcrypt_hash =
        PasswordHash::parse("$2b$12$e8Y7Yp3P9D/w/G5eH9T1eeG3Q1.2K8eG3Q1.2K8eG3Q1.2K8eG3Q1")
            .unwrap();
    assert_eq!(bcrypt_hash.algorithm(), Algorithm::Bcrypt);

    let pbkdf2_hash = PasswordHash::parse(
        "$pbkdf2-sha256$i=600000,l=32$c29tZXNhbHQ$RGF0YWJhc2VTZWNyZXRLZXlIYXNoVmFsdWU",
    )
    .unwrap();
    assert_eq!(pbkdf2_hash.algorithm(), Algorithm::Pbkdf2Sha256);

    let noop_hash = PasswordHash::parse("$noop$secret123").unwrap();
    assert_eq!(noop_hash.algorithm(), Algorithm::Noop);

    let invalid = PasswordHash::parse("invalid_hash_string");
    assert!(matches!(invalid, Err(PasswordError::InvalidFormat(_))));
}

#[test]
fn test_noop_hasher() {
    let hasher = NoopHasher::new();
    assert_eq!(hasher.algorithm(), Algorithm::Noop);

    let password = "my_super_secret_password";
    let hash = hasher.hash_password(password).unwrap();

    assert_eq!(hash.as_str(), "$noop$my_super_secret_password");
    assert_eq!(hash.algorithm(), Algorithm::Noop);

    assert!(hasher.verify_password(password, &hash).unwrap());
    assert!(!hasher.verify_password("wrong_password", &hash).unwrap());
}

#[test]
fn test_argon2_hasher() {
    let hasher = Argon2Hasher::new();
    assert_eq!(hasher.algorithm(), Algorithm::Argon2id);

    let password = "secure_user_pass_123!";
    let hash = hasher.hash_password(password).unwrap();

    assert_eq!(hash.algorithm(), Algorithm::Argon2id);
    assert!(hash.as_str().starts_with("$argon2id$"));

    assert!(hasher.verify_password(password, &hash).unwrap());
    assert!(!hasher.verify_password("wrong_pass", &hash).unwrap());
}

#[test]
fn test_bcrypt_hasher() {
    let hasher = BcryptHasher::new();
    assert_eq!(hasher.algorithm(), Algorithm::Bcrypt);

    let password = "bcrypt_secret_password";
    let hash = hasher.hash_password(password).unwrap();

    assert_eq!(hash.algorithm(), Algorithm::Bcrypt);
    assert!(hash.as_str().starts_with("$2b$"));

    assert!(hasher.verify_password(password, &hash).unwrap());
    assert!(!hasher.verify_password("incorrect_pass", &hash).unwrap());
}

#[cfg(feature = "pbkdf2")]
#[test]
fn test_pbkdf2_hasher() {
    let hasher = Pbkdf2Hasher::new();
    assert_eq!(hasher.algorithm(), Algorithm::Pbkdf2Sha256);

    let password = "pbkdf2_secret_password";
    let hash = hasher.hash_password(password).unwrap();

    assert_eq!(hash.algorithm(), Algorithm::Pbkdf2Sha256);
    assert!(hash.as_str().starts_with("$pbkdf2-sha256$"));

    assert!(hasher.verify_password(password, &hash).unwrap());
    assert!(!hasher.verify_password("incorrect_pass", &hash).unwrap());
}

#[test]
fn test_hasher_manager_routing_and_rehash() {
    let manager = PasswordHasherManager::builder()
        .with_hasher(Arc::new(Argon2Hasher::new()))
        .with_hasher(Arc::new(BcryptHasher::new()))
        .with_hasher(Arc::new(NoopHasher::new()))
        .default_algorithm(Algorithm::Argon2id)
        .build()
        .unwrap();

    assert_eq!(manager.default_algorithm(), Algorithm::Argon2id);

    let password = "password_for_migration_test";

    // 1. Hash with Argon2id (default)
    let argon_hash = manager.hash_password(password).unwrap();
    assert_eq!(argon_hash.algorithm(), Algorithm::Argon2id);
    assert!(!manager.needs_rehash(&argon_hash));
    assert!(manager.verify_password(password, &argon_hash).unwrap());

    // 2. Old Bcrypt hash verification & rehash detection
    let bcrypt_hasher = BcryptHasher::new();
    let bcrypt_hash = bcrypt_hasher.hash_password(password).unwrap();
    assert_eq!(bcrypt_hash.algorithm(), Algorithm::Bcrypt);

    // Manager should detect that bcrypt hash needs re-hashing
    assert!(manager.needs_rehash(&bcrypt_hash));
    // But manager should still verify bcrypt hash successfully by auto-detecting algorithm
    assert!(manager.verify_password(password, &bcrypt_hash).unwrap());
    assert!(
        manager
            .verify_password_str(password, bcrypt_hash.as_str())
            .unwrap()
    );

    // 3. Old Noop hash verification
    let noop_hasher = NoopHasher::new();
    let noop_hash = noop_hasher.hash_password(password).unwrap();
    assert!(manager.needs_rehash(&noop_hash));
    assert!(manager.verify_password(password, &noop_hash).unwrap());
}

#[tokio::test]
async fn test_async_password_hasher() {
    let manager = PasswordHasherManager::default();
    let password = "async_computation_password_456!".to_string();

    let hash = manager.hash_password_async(password.clone()).await.unwrap();

    assert!(
        manager
            .verify_password_async(password.clone(), hash.clone())
            .await
            .unwrap()
    );

    assert!(
        !manager
            .verify_password_async("wrong_pass".to_string(), hash)
            .await
            .unwrap()
    );
}
