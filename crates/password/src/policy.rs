use serde::{Deserialize, Serialize};

use crate::error::PasswordError;

/// Configurable password policy enforcing complexity and length constraints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasswordPolicy {
    /// Minimum allowed character length.
    pub min_length: usize,
    /// Maximum allowed character length.
    pub max_length: usize,
    /// Whether at least one uppercase letter is required.
    pub require_uppercase: bool,
    /// Whether at least one lowercase letter is required.
    pub require_lowercase: bool,
    /// Whether at least one numeric digit is required.
    pub require_digit: bool,
    /// Whether at least one special non-alphanumeric character is required.
    pub require_special: bool,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            min_length: 8,
            max_length: 128,
            require_uppercase: true,
            require_lowercase: true,
            require_digit: true,
            require_special: false,
        }
    }
}

impl PasswordPolicy {
    /// Validates a plaintext password against the configured policy rules.
    pub fn validate(&self, password: &str) -> Result<(), PasswordError> {
        if password.len() < self.min_length {
            return Err(PasswordError::TooShort {
                min: self.min_length,
            });
        }

        if password.len() > self.max_length {
            return Err(PasswordError::TooLong {
                max: self.max_length,
            });
        }

        if self.require_uppercase && !password.chars().any(|c| c.is_uppercase()) {
            return Err(PasswordError::MissingUppercase);
        }

        if self.require_lowercase && !password.chars().any(|c| c.is_lowercase()) {
            return Err(PasswordError::MissingLowercase);
        }

        if self.require_digit && !password.chars().any(|c| c.is_ascii_digit()) {
            return Err(PasswordError::MissingDigit);
        }

        if self.require_special && !password.chars().any(|c| !c.is_alphanumeric()) {
            return Err(PasswordError::MissingSpecialChar);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_default_policy() {
        let policy = PasswordPolicy::default();

        assert_eq!(
            policy.validate("short1A"),
            Err(PasswordError::TooShort { min: 8 })
        );
        assert_eq!(
            policy.validate("nouppercase123"),
            Err(PasswordError::MissingUppercase)
        );
        assert_eq!(
            policy.validate("NOLOWERCASE123"),
            Err(PasswordError::MissingLowercase)
        );
        assert_eq!(
            policy.validate("NoDigitsHere"),
            Err(PasswordError::MissingDigit)
        );
        assert!(policy.validate("ValidPass123").is_ok());
    }

    #[test]
    fn validates_special_characters_when_required() {
        let policy = PasswordPolicy {
            require_special: true,
            ..Default::default()
        };

        assert_eq!(
            policy.validate("ValidPass123"),
            Err(PasswordError::MissingSpecialChar)
        );
        assert!(policy.validate("ValidPass123!").is_ok());
    }
}
