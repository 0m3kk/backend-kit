#![allow(clippy::unwrap_used, clippy::expect_used)]

use secret_store::memory::MemorySecretStore;
use secret_store::{CipherAlgorithm, KEY_LEN, KeyRing, MasterKey};
use std::sync::Arc;
use two_factor_auth_totp::{
    TotpAlgorithm, TotpConfig, TotpDigits, TotpTwoFactorAuth, TwoFactorMethod, TwoFactorProvider,
    TwoFactorResponse,
};

fn create_test_secret_store() -> Arc<MemorySecretStore> {
    let master_key = MasterKey::new(1, [9u8; KEY_LEN]);
    let keyring = KeyRing::new([master_key]).unwrap();
    Arc::new(MemorySecretStore::new(keyring, CipherAlgorithm::Aes256Gcm))
}

#[tokio::test]
async fn test_totp_token_generation_and_verification() {
    let store = create_test_secret_store();
    let service = TotpTwoFactorAuth::new(store);
    let secret = service.generate_secret().unwrap();

    let timestamp = 1700000000u64;
    let token = service.generate_token(&secret, timestamp).unwrap();

    assert_eq!(token.len(), 6);
    assert!(token.chars().all(|c| c.is_ascii_digit()));

    // Verify exact timestamp match
    let is_valid = service.verify_token(&secret, &token, timestamp, 0).unwrap();
    assert!(is_valid);

    // Verify invalid token fails
    let is_valid = service
        .verify_token(&secret, "000000", timestamp, 0)
        .unwrap();
    assert!(!is_valid);
}

#[tokio::test]
async fn test_totp_secret_store_integration() {
    let store = create_test_secret_store();
    let service = TotpTwoFactorAuth::new(store);
    let user_id = "user_secret_store_test";

    // 1. Enroll user in SecretStore
    let (secret, url) = service.enroll_user(user_id).await.unwrap();
    assert!(!secret.as_base32().is_empty());
    assert!(url.contains("secret="));

    // 2. Generate token and verify against SecretStore
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let token = service.generate_token(&secret, now).unwrap();

    let is_valid = service
        .verify_user_token(user_id, &token, now)
        .await
        .unwrap();
    assert!(is_valid);

    // Invalid token verification fails
    let invalid = service
        .verify_user_token(user_id, "000000", now)
        .await
        .unwrap();
    assert!(!invalid);

    // 3. Revoke user
    let revoked = service.revoke_user(user_id).await.unwrap();
    assert!(revoked);

    let verify_after_revoke = service
        .verify_user_token(user_id, &token, now)
        .await
        .unwrap();
    assert!(!verify_after_revoke);
}

#[tokio::test]
async fn test_generic_two_factor_provider_interface() {
    let store = create_test_secret_store();
    let service = TotpTwoFactorAuth::new(store);
    assert_eq!(service.method(), TwoFactorMethod::Totp);

    let challenge = service.issue_challenge("user_42").await.unwrap();
    assert_eq!(challenge.method, TwoFactorMethod::Totp);
    assert!(challenge.payload.is_some());

    let response = TwoFactorResponse::totp("123456");
    let valid = service.verify_response("user_42", &response).await;
    assert!(valid.is_ok());
}

#[tokio::test]
async fn test_totp_skew_tolerance() {
    let store = create_test_secret_store();
    let service = TotpTwoFactorAuth::new(store);
    let secret = service.generate_secret().unwrap();

    let t0 = 1700000000u64; // base step
    let token = service.generate_token(&secret, t0).unwrap();

    // 30 seconds later (1 step forward)
    let t1 = t0 + 30;

    // With skew_windows = 0, t1 verification fails for t0 token
    let valid_no_skew = service.verify_token(&secret, &token, t1, 0).unwrap();
    assert!(!valid_no_skew);

    // With skew_windows = 1, t1 verification succeeds
    let valid_skew_1 = service.verify_token(&secret, &token, t1, 1).unwrap();
    assert!(valid_skew_1);
}

#[tokio::test]
async fn test_totp_custom_config() {
    let store = create_test_secret_store();
    let config = TotpConfig::builder()
        .algorithm(TotpAlgorithm::Sha256)
        .digits(TotpDigits::Eight)
        .step(60)
        .issuer("TestOrg")
        .account_name("user@test.org")
        .build();

    let service = TotpTwoFactorAuth::with_config(store, config.clone());
    let secret = service.generate_secret().unwrap();

    let timestamp = 1700000000u64;
    let token = service.generate_token(&secret, timestamp).unwrap();

    assert_eq!(token.len(), 8);

    let is_valid = service.verify_token(&secret, &token, timestamp, 0).unwrap();
    assert!(is_valid);

    let url = service.build_otpauth_url(&secret, &config).unwrap();
    assert!(url.contains("secret="));
    assert!(url.contains("issuer=TestOrg"));
}

#[cfg(feature = "qr")]
#[tokio::test]
async fn test_totp_qr_generation() {
    let store = create_test_secret_store();
    let service = TotpTwoFactorAuth::new(store);
    let secret = service.generate_secret().unwrap();

    let qr_base64 = service.generate_qr_base64(&secret).unwrap();
    assert!(!qr_base64.is_empty());

    let qr_png = service.generate_qr_png(&secret).unwrap();
    assert!(!qr_png.is_empty());
}
