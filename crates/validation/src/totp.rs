//! TOTP and two-factor authentication validation functions.

use validator::ValidationError;

/// Validates that a string slice is a valid 6-digit TOTP verification code (ASCII digits only).
pub fn validate_totp_code(code: &str) -> Result<(), ValidationError> {
    if code.len() == 6 && code.chars().all(|c| c.is_ascii_digit()) {
        Ok(())
    } else {
        let mut err = ValidationError::new("invalid_totp_code");
        err.message = Some("TOTP verification code must be exactly 6 digits".into());
        Err(err)
    }
}

/// Validates that a string slice is a non-empty backup recovery code (alphanumeric and dashes).
pub fn validate_recovery_code(code: &str) -> Result<(), ValidationError> {
    let trimmed = code.trim();
    if !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        Ok(())
    } else {
        let mut err = ValidationError::new("invalid_recovery_code");
        err.message = Some("Recovery code must be alphanumeric with dashes".into());
        Err(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_totp_code_successfully() {
        assert!(validate_totp_code("123456").is_ok());
        assert!(validate_totp_code("000000").is_ok());
        assert!(validate_totp_code("999999").is_ok());
    }

    #[test]
    fn rejects_invalid_totp_code() {
        assert!(validate_totp_code("12345").is_err());
        assert!(validate_totp_code("1234567").is_err());
        assert!(validate_totp_code("12345a").is_err());
        assert!(validate_totp_code("abcdef").is_err());
        assert!(validate_totp_code("      ").is_err());
        assert!(validate_totp_code("").is_err());
    }

    #[test]
    fn validates_recovery_code_successfully() {
        assert!(validate_recovery_code("abcd-1234-efgh").is_ok());
        assert!(validate_recovery_code("ABCD-1234-EFGH").is_ok());
        assert!(validate_recovery_code("12345678").is_ok());
    }

    #[test]
    fn rejects_invalid_recovery_code() {
        assert!(validate_recovery_code("").is_err());
        assert!(validate_recovery_code("   ").is_err());
        assert!(validate_recovery_code("abcd_1234").is_err());
        assert!(validate_recovery_code("abcd!1234").is_err());
    }
}
