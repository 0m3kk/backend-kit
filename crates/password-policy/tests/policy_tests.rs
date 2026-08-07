use password_policy::{
    PasswordGenerator, PasswordPolicy, PasswordStrength, PolicyError, PolicyViolation, UserContext,
};

#[test]
fn test_default_policy_validation() {
    let policy = PasswordPolicy::default();

    // Valid password
    assert!(policy.validate("P@ssword123").is_ok());

    // Too short
    let err = policy.validate("P@1a").unwrap_err();
    if let PolicyError::Violations(v) = err {
        assert!(
            v.iter()
                .any(|item| matches!(item, PolicyViolation::TooShort { .. }))
        );
    } else {
        panic!("Expected PolicyError::Violations");
    }

    // Missing uppercase
    let err = policy.validate("p@ssword123").unwrap_err();
    if let PolicyError::Violations(v) = err {
        assert!(
            v.iter()
                .any(|item| matches!(item, PolicyViolation::InsufficientUppercase { .. }))
        );
    }

    // Missing digit
    let err = policy.validate("P@sswordabc").unwrap_err();
    if let PolicyError::Violations(v) = err {
        assert!(
            v.iter()
                .any(|item| matches!(item, PolicyViolation::InsufficientDigits { .. }))
        );
    }

    // Missing special character
    let err = policy.validate("Password123").unwrap_err();
    if let PolicyError::Violations(v) = err {
        assert!(
            v.iter()
                .any(|item| matches!(item, PolicyViolation::InsufficientSpecial { .. }))
        );
    }
}

#[test]
fn test_nist_preset() {
    let policy = PasswordPolicy::nist();

    // NIST allows spaces and simple passwords as long as min length is met & clean of blocklist
    assert!(policy.validate("correct horse battery staple").is_ok());

    // Blocklist check
    let report = policy.audit("password123");
    assert!(!report.is_valid);
    assert!(report.violations.contains(&PolicyViolation::CommonPassword));
}

#[test]
fn test_owasp_preset() {
    let policy = PasswordPolicy::owasp();
    assert!(policy.validate("SecureP@ssw0rd!").is_ok());
    assert!(policy.validate("short").is_err());
}

#[test]
fn test_strict_preset() {
    let policy = PasswordPolicy::strict();

    // Compliant strict password
    assert!(policy.validate("K9#mQ!2vL$9xP@7w").is_ok());

    // Fails due to sequential pattern
    let report = policy.audit("K9#mQ!2vL$12345@7w");
    assert!(!report.is_valid);
    assert!(
        report
            .violations
            .iter()
            .any(|v| matches!(v, PolicyViolation::SequentialPatternDetected { .. }))
    );

    // Fails due to repetitive character
    let report = policy.audit("K9#mQ!2vL$$$$P@7w");
    assert!(!report.is_valid);
    assert!(
        report
            .violations
            .iter()
            .any(|v| matches!(v, PolicyViolation::RepetitivePatternDetected { .. }))
    );
}

#[test]
fn test_user_context_validation() {
    let policy = PasswordPolicy::default();
    let ctx = UserContext::new()
        .with_username("johndoe")
        .with_email("john.doe@acme.com")
        .with_name("John", "Doe");

    // Password containing username
    let report = policy.audit_with_context("MyJohnDoePass1!", &ctx);
    assert!(!report.is_valid);
    assert!(
        report
            .violations
            .iter()
            .any(|v| matches!(v, PolicyViolation::ContainsUserContext { .. }))
    );

    // Password containing email token
    let report = policy.audit_with_context("AcmeCorp2026!", &ctx);
    assert!(!report.is_valid);
    assert!(
        report
            .violations
            .iter()
            .any(|v| matches!(v, PolicyViolation::ContainsUserContext { .. }))
    );

    // Clean password
    let report = policy.audit_with_context("SuperUnrelatedPass99!", &ctx);
    assert!(report.is_valid);
}

#[test]
fn test_entropy_and_strength() {
    let (entropy, strength) = (
        PasswordPolicy::calculate_entropy("short"),
        PasswordStrength::from_entropy(PasswordPolicy::calculate_entropy("short")),
    );
    assert_eq!(strength, PasswordStrength::Weak);
    assert!(entropy < 36.0);

    let entropy = PasswordPolicy::calculate_entropy("K9#mQ!2vL$9xP@7w");
    let strength = PasswordStrength::from_entropy(entropy);
    assert!(strength >= PasswordStrength::Strong);
}

#[test]
fn test_password_generator() {
    let policy = PasswordPolicy::strict();
    let generator = PasswordGenerator::new();

    let password = generator.generate(&policy).expect("Generation failed");
    assert!(password.len() >= 16);
    assert!(policy.validate(&password).is_ok());

    let custom_len_password = generator
        .generate_with_length(&policy, 24)
        .expect("Generation failed");
    assert_eq!(custom_len_password.len(), 24);
    assert!(policy.validate(&custom_len_password).is_ok());
}

#[test]
fn test_builder_invalid_config() {
    let result = PasswordPolicy::builder()
        .min_length(20)
        .max_length(10)
        .build();
    assert!(result.is_err());
}
