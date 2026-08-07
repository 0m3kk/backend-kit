#![allow(clippy::unwrap_used, clippy::expect_used)]

use secret_store::memory::MemorySecretStore;
use secret_store::{CipherAlgorithm, KEY_LEN, KeyRing, MasterKey};
use sms_sender::MemorySmsSender;
use std::sync::Arc;
use two_factor_auth_sms::{
    SmsOtpConfig, SmsTwoFactorAuth, TwoFactorMethod, TwoFactorProvider, TwoFactorResponse,
};

fn create_test_setup() -> (Arc<MemorySecretStore>, Arc<MemorySmsSender>) {
    let master_key = MasterKey::new(1, [8u8; KEY_LEN]);
    let keyring = KeyRing::new([master_key]).unwrap();
    let store = Arc::new(MemorySecretStore::new(keyring, CipherAlgorithm::Aes256Gcm));
    let sms_sender = Arc::new(MemorySmsSender::new());
    (store, sms_sender)
}

#[tokio::test]
async fn test_sms_enrollment_and_sending() {
    let (store, sms_sender) = create_test_setup();
    let service = SmsTwoFactorAuth::new(store, sms_sender.clone());
    let user_id = "user_sms_1";
    let phone = "+15551234567";

    // 1. Enroll user
    service.enroll_user(user_id, phone).await.unwrap();
    let fetched = service.get_user_phone(user_id).await.unwrap();
    assert_eq!(fetched.as_deref(), Some(phone));

    // 2. Send code
    let masked = service.send_code(user_id).await.unwrap();
    assert!(masked.contains("4567"));

    // 3. Verify SMS was dispatched to MemorySmsSender
    let last_msg = sms_sender.last_message_for(phone).await.unwrap();
    assert!(last_msg.body.contains("Your verification code is:"));

    // Extract code from message
    let code = last_msg.body.split(':').nth(1).unwrap().trim();
    assert_eq!(code.len(), 6);

    // 4. Verify code
    let is_valid = service.verify_code(user_id, code).await.unwrap();
    assert!(is_valid);

    // Re-verification fails (consumed)
    let re_verify = service.verify_code(user_id, code).await.unwrap();
    assert!(!re_verify);
}

#[tokio::test]
async fn test_sms_custom_config() {
    let (store, sms_sender) = create_test_setup();
    let config = SmsOtpConfig {
        code_length: 8,
        ttl: std::time::Duration::from_secs(60),
        message_template: "OTP Code: {}".to_string(),
    };
    let service = SmsTwoFactorAuth::with_config(store, sms_sender.clone(), config);
    let user_id = "user_sms_2";
    let phone = "+15559876543";

    service.enroll_user(user_id, phone).await.unwrap();
    service.send_code(user_id).await.unwrap();

    let msg = sms_sender.last_message_for(phone).await.unwrap();
    let code = msg.body.split(':').nth(1).unwrap().trim();
    assert_eq!(code.len(), 8);

    assert!(service.verify_code(user_id, code).await.unwrap());
}

#[tokio::test]
async fn test_sms_generic_two_factor_provider() {
    let (store, sms_sender) = create_test_setup();
    let service = SmsTwoFactorAuth::new(store, sms_sender.clone());
    assert_eq!(service.method(), TwoFactorMethod::SmsOtp);

    let user_id = "user_sms_3";
    let phone = "+15550001111";
    service.enroll_user(user_id, phone).await.unwrap();

    // Issue challenge
    let challenge = service.issue_challenge(user_id).await.unwrap();
    assert_eq!(challenge.method, TwoFactorMethod::SmsOtp);

    let msg = sms_sender.last_message_for(phone).await.unwrap();
    let code = msg.body.split(':').nth(1).unwrap().trim();

    let response = TwoFactorResponse::sms_otp(code);
    let valid = service.verify_response(user_id, &response).await.unwrap();
    assert!(valid);
}
