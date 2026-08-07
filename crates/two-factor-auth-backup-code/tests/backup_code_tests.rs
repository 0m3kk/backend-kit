#![allow(clippy::unwrap_used, clippy::expect_used)]

use secret_store::memory::MemorySecretStore;
use secret_store::{CipherAlgorithm, KEY_LEN, KeyRing, MasterKey};
use std::sync::Arc;
use two_factor_auth_backup_code::{
    BackupCodeTwoFactorAuth, TwoFactorMethod, TwoFactorProvider, TwoFactorResponse,
};

fn create_test_secret_store() -> Arc<MemorySecretStore> {
    let master_key = MasterKey::new(1, [9u8; KEY_LEN]);
    let keyring = KeyRing::new([master_key]).unwrap();
    Arc::new(MemorySecretStore::new(keyring, CipherAlgorithm::Aes256Gcm))
}

#[tokio::test]
async fn test_backup_code_generation_and_hashing() {
    let store = create_test_secret_store();
    let service = BackupCodeTwoFactorAuth::new(store);
    let set = service.generate_codes(8);

    assert_eq!(set.plain_codes.len(), 8);
    assert_eq!(set.hashed_codes.len(), 8);

    for (plain, hashed) in set.plain_codes.iter().zip(set.hashed_codes.iter()) {
        assert_eq!(&service.hash_code(plain), hashed);
    }
}

#[tokio::test]
async fn test_backup_code_secret_store_integration() {
    let store = create_test_secret_store();
    let service = BackupCodeTwoFactorAuth::new(store);
    let user_id = "user_backup_secret_store_test";

    // 1. Enroll user (stores hashed codes encrypted in SecretStore)
    let set = service.enroll_user(user_id, 3).await.unwrap();
    assert_eq!(set.plain_codes.len(), 3);

    // 2. Verify and consume 1st code
    let code1 = &set.plain_codes[0];
    let is_valid = service
        .verify_and_consume_user_code(user_id, code1)
        .await
        .unwrap();
    assert!(is_valid);

    // Using code1 again fails (consumed)
    let code1_again = service
        .verify_and_consume_user_code(user_id, code1)
        .await
        .unwrap();
    assert!(!code1_again);

    // 3. Consume remaining 2 codes
    assert!(
        service
            .verify_and_consume_user_code(user_id, &set.plain_codes[1])
            .await
            .unwrap()
    );
    assert!(
        service
            .verify_and_consume_user_code(user_id, &set.plain_codes[2])
            .await
            .unwrap()
    );

    // Store entry is now empty and auto-deleted
    let verify_empty = service
        .verify_and_consume_user_code(user_id, &set.plain_codes[2])
        .await
        .unwrap();
    assert!(!verify_empty);
}

#[tokio::test]
async fn test_backup_code_verification_and_consumption() {
    let store = create_test_secret_store();
    let service = BackupCodeTwoFactorAuth::new(store);
    let set = service.generate_codes(5);

    let submitted_code = &set.plain_codes[2];

    // Verify and consume valid code
    let remaining = service
        .verify_and_consume(submitted_code, &set.hashed_codes)
        .unwrap();

    assert!(remaining.is_some());
    let remaining = remaining.unwrap();
    assert_eq!(remaining.len(), 4);

    // Verify consuming the same code again fails (already used)
    let remaining_again = service
        .verify_and_consume(submitted_code, &remaining)
        .unwrap();
    assert!(remaining_again.is_none());

    // Verify invalid code fails
    let invalid = service
        .verify_and_consume("INVALID-CODE", &set.hashed_codes)
        .unwrap();
    assert!(invalid.is_none());
}

#[tokio::test]
async fn test_backup_code_generic_provider() {
    let store = create_test_secret_store();
    let service = BackupCodeTwoFactorAuth::new(store);
    assert_eq!(service.method(), TwoFactorMethod::BackupCode);

    let set = service.enroll_user("user_99", 3).await.unwrap();

    let challenge = service.issue_challenge("user_99").await.unwrap();
    assert_eq!(challenge.method, TwoFactorMethod::BackupCode);

    let response = TwoFactorResponse::backup_code(&set.plain_codes[0]);
    let valid = service.verify_response("user_99", &response).await.unwrap();
    assert!(valid);
}
