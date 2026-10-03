//! Tenant validation functions.

use validator::ValidationError;

use crate::validate_uuid_v7;

/// Validates that a string slice is a valid tenant identifier (either "default" or a valid UUIDv7).
pub fn validate_tenant_id(id: &str) -> Result<(), ValidationError> {
    if id == "default" {
        return Ok(());
    }
    validate_uuid_v7(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_default_tenant_id() {
        assert!(validate_tenant_id("default").is_ok());
    }

    #[test]
    fn validates_uuid_v7_tenant_id() {
        let valid_v7 = uuid::Uuid::now_v7().to_string();
        assert!(validate_tenant_id(&valid_v7).is_ok());
    }

    #[test]
    fn rejects_invalid_tenant_id() {
        assert!(validate_tenant_id("invalid-id").is_err());
        assert!(validate_tenant_id("").is_err());
        assert!(validate_tenant_id("00000000-0000-4000-8000-000000000000").is_err());
    }
}
