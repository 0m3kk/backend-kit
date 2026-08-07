#![allow(clippy::unwrap_used, clippy::expect_used)]

use email_sender::EmailAddress;
use email_sender::memory::MemoryEmailSender;
use secret_store::memory::MemorySecretStore;
use secret_store::{CipherAlgorithm, KEY_LEN, KeyRing, MasterKey};
use std::sync::Arc;
use two_factor_auth_email::{
    EmailOtpConfig, EmailTwoFactorAuth, TwoFactorMethod, TwoFactorProvider, TwoFactorResponse,
};

fn create_test_setup() -> (Arc<MemorySecretStore>, Arc<MemoryEmailSender>, EmailAddress) {
    let master_key = MasterKey::new(1, [7u8; KEY_LEN]);
    let keyring = KeyRing::new([master_key]).unwrap();
    let store = Arc::new(MemorySecretStore::new(keyring, CipherAlgorithm::Aes256Gcm));
    let email_sender = Arc::new(MemoryEmailSender::new());
    let sender_addr = EmailAddress::new("no-reply@example.com").unwrap();
    (store, email_sender, sender_addr)
}

#[tokio::test]
async fn test_email_enrollment_and_sending() {
    let (store, email_sender, sender_addr) = create_test_setup();
    let service = EmailTwoFactorAuth::new(store, email_sender.clone(), sender_addr);
    let user_id = "user_email_1";
    let recipient = "alice@example.com";

    // 1. Enroll user
    service.enroll_user(user_id, recipient).await.unwrap();
    let fetched = service.get_user_email(user_id).await.unwrap();
    assert_eq!(fetched.as_deref(), Some(recipient));

    // 2. Send code
    let masked = service.send_code(user_id).await.unwrap();
    assert!(masked.contains("@example.com"));

    // 3. Verify Email was dispatched to MemoryEmailSender
    let sent = email_sender.sent_emails().await;
    assert_eq!(sent.len(), 1);
    let email = &sent[0];
    assert_eq!(email.to[0].email, recipient);
    assert_eq!(email.subject, "Your Verification Code");

    // Extract code from text body
    let body = email.text_body.as_ref().unwrap();
    let code = body.split(':').nth(1).unwrap().trim();
    assert_eq!(code.len(), 6);

    // 4. Verify code
    let is_valid = service.verify_code(user_id, code).await.unwrap();
    assert!(is_valid);

    // Re-verification fails (consumed)
    let re_verify = service.verify_code(user_id, code).await.unwrap();
    assert!(!re_verify);
}

#[tokio::test]
async fn test_email_custom_config() {
    let (store, email_sender, sender_addr) = create_test_setup();
    let config = EmailOtpConfig {
        code_length: 8,
        ttl: std::time::Duration::from_secs(120),
        subject: "Security Login Code".to_string(),
        html_template: "<p>Code: {}</p>".to_string(),
        text_template: "Code: {}".to_string(),
    };

    let service = EmailTwoFactorAuth::with_config(store, email_sender.clone(), sender_addr, config);
    let user_id = "user_email_2";
    let recipient = "bob@example.com";

    service.enroll_user(user_id, recipient).await.unwrap();
    service.send_code(user_id).await.unwrap();

    let sent = email_sender.sent_emails().await;
    let email = &sent[0];
    assert_eq!(email.subject, "Security Login Code");

    let code = email
        .text_body
        .as_ref()
        .unwrap()
        .split(':')
        .nth(1)
        .unwrap()
        .trim();
    assert_eq!(code.len(), 8);

    assert!(service.verify_code(user_id, code).await.unwrap());
}

#[tokio::test]
async fn test_email_generic_two_factor_provider() {
    let (store, email_sender, sender_addr) = create_test_setup();
    let service = EmailTwoFactorAuth::new(store, email_sender.clone(), sender_addr);
    assert_eq!(service.method(), TwoFactorMethod::EmailOtp);

    let user_id = "user_email_3";
    let recipient = "carol@example.com";
    service.enroll_user(user_id, recipient).await.unwrap();

    // Issue challenge
    let challenge = service.issue_challenge(user_id).await.unwrap();
    assert_eq!(challenge.method, TwoFactorMethod::EmailOtp);

    let sent = email_sender.sent_emails().await;
    let email = &sent[0];
    let code = email
        .text_body
        .as_ref()
        .unwrap()
        .split(':')
        .nth(1)
        .unwrap()
        .trim();

    let response = TwoFactorResponse::email_otp(code);
    let valid = service.verify_response(user_id, &response).await.unwrap();
    assert!(valid);
}
