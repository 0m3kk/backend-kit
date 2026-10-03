use thiserror::Error;

/// Password validation and cryptography errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PasswordError {
    /// Password length is below minimum required characters.
    #[error("Password is too short (minimum {min} characters required)")]
    TooShort { min: usize },

    /// Password length exceeds maximum allowed characters.
    #[error("Password is too long (maximum {max} characters allowed)")]
    TooLong { max: usize },

    /// Password does not contain at least one uppercase letter.
    #[error("Password must contain at least one uppercase letter")]
    MissingUppercase,

    /// Password does not contain at least one lowercase letter.
    #[error("Password must contain at least one lowercase letter")]
    MissingLowercase,

    /// Password does not contain at least one numeric digit.
    #[error("Password must contain at least one digit")]
    MissingDigit,

    /// Password does not contain at least one special symbol.
    #[error("Password must contain at least one special character")]
    MissingSpecialChar,

    /// Failure during password hashing.
    #[error("Password hashing failed: {0}")]
    Hashing(String),

    /// Failure during password verification.
    #[error("Password verification failed: {0}")]
    Verification(String),
}
