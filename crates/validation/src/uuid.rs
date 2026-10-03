//! UUID validation functions.

use validator::ValidationError;

/// Validates that a string slice is a valid UUIDv7 format.
pub fn validate_uuid_v7(id: &str) -> Result<(), ValidationError> {
    match uuid::Uuid::parse_str(id) {
        Ok(parsed) if parsed.get_version_num() == 7 => Ok(()),
        _ => Err(ValidationError::new("invalid_uuid_v7")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_uuid_v7_successfully() {
        let valid_v7 = uuid::Uuid::now_v7().to_string();
        assert!(validate_uuid_v7(&valid_v7).is_ok());
    }

    #[test]
    fn rejects_non_v7_uuid() {
        let v4 = "00000000-0000-4000-8000-000000000000";
        assert!(validate_uuid_v7(v4).is_err());
    }

    #[test]
    fn rejects_invalid_string() {
        assert!(validate_uuid_v7("invalid-uuid-string").is_err());
        assert!(validate_uuid_v7("").is_err());
    }
}
