use two_factor_auth::{
    BackupCode, MemoryTwoFactorAuth, TwoFactorMethod, TwoFactorProvider, TwoFactorResponse,
};

#[tokio::test]
async fn test_memory_two_factor_auth_provider() {
    let service = MemoryTwoFactorAuth::new();
    assert_eq!(service.method(), TwoFactorMethod::Totp);

    let challenge = service.issue_challenge("alice").await.unwrap();
    assert_eq!(challenge.challenge_id, "mem_chal_alice");

    let response = TwoFactorResponse::totp("123456");
    let valid = service
        .verify_response(&challenge.challenge_id, &response)
        .await
        .unwrap();
    assert!(valid);

    let invalid_response = TwoFactorResponse::totp("000000");
    let valid = service
        .verify_response(&challenge.challenge_id, &invalid_response)
        .await
        .unwrap();
    assert!(!valid);

    // Preset custom response
    service.preset_response("custom_chal", "654321");
    let valid = service
        .verify_response("custom_chal", &TwoFactorResponse::totp("654321"))
        .await
        .unwrap();
    assert!(valid);
}

#[test]
fn test_backup_code_generation() {
    let codes = BackupCode::generate_set(5);
    assert_eq!(codes.len(), 5);

    for code in &codes {
        assert_eq!(code.len(), 9); // XXXX-XXXX format
        assert_eq!(BackupCode::normalize(code).len(), 8);
    }
}

#[test]
fn test_two_factor_method_display() {
    assert_eq!(format!("{}", TwoFactorMethod::Totp), "TOTP");
    assert_eq!(format!("{}", TwoFactorMethod::SmsOtp), "SMS_OTP");
    assert_eq!(format!("{}", TwoFactorMethod::EmailOtp), "EMAIL_OTP");
    assert_eq!(format!("{}", TwoFactorMethod::BackupCode), "BACKUP_CODE");
}

#[test]
fn test_two_factor_challenge_with_expiration() {
    use two_factor_auth::TwoFactorChallenge;

    let challenge = TwoFactorChallenge::new("chal_123", TwoFactorMethod::Totp)
        .with_payload("setup_url")
        .with_expiration(1700000000);

    assert_eq!(challenge.challenge_id, "chal_123");
    assert_eq!(challenge.method, TwoFactorMethod::Totp);
    assert_eq!(challenge.payload, Some("setup_url".to_string()));
    assert_eq!(challenge.expires_at, Some(1700000000));
}

#[test]
fn test_two_factor_challenge_without_optional_fields() {
    use two_factor_auth::TwoFactorChallenge;

    let challenge = TwoFactorChallenge::new("chal_456", TwoFactorMethod::EmailOtp);

    assert_eq!(challenge.challenge_id, "chal_456");
    assert_eq!(challenge.method, TwoFactorMethod::EmailOtp);
    assert!(challenge.payload.is_none());
    assert!(challenge.expires_at.is_none());
}

#[test]
fn test_two_factor_response_constructors() {
    let totp = TwoFactorResponse::totp("123456");
    assert_eq!(totp.method, TwoFactorMethod::Totp);
    assert_eq!(totp.response_data, "123456");

    let sms = TwoFactorResponse::sms_otp("654321");
    assert_eq!(sms.method, TwoFactorMethod::SmsOtp);
    assert_eq!(sms.response_data, "654321");

    let email = TwoFactorResponse::email_otp("111222");
    assert_eq!(email.method, TwoFactorMethod::EmailOtp);
    assert_eq!(email.response_data, "111222");

    let backup = TwoFactorResponse::backup_code("ABCD-EFGH");
    assert_eq!(backup.method, TwoFactorMethod::BackupCode);
    assert_eq!(backup.response_data, "ABCD-EFGH");
}

#[test]
fn test_backup_code_normalize() {
    assert_eq!(BackupCode::normalize("abcd-efgh"), "ABCDEFGH");
    assert_eq!(BackupCode::normalize("  ABCD EFGH  "), "ABCDEFGH");
    assert_eq!(BackupCode::normalize("AB-CD EF-GH"), "ABCDEFGH");
}
