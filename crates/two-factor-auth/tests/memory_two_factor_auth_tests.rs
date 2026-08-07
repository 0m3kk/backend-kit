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
