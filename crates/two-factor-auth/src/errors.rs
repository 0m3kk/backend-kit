use thiserror::Error;

/// Errors that can occur during 2FA / TOTP operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TwoFactorError {
    /// Invalid base32 or raw secret format.
    #[error("Invalid secret: {0}")]
    InvalidSecret(String),

    /// Invalid TOTP token length, characters, or value.
    #[error("Invalid token: {0}")]
    InvalidToken(String),

    /// Invalid number of digits specified (supported: 6, 7, 8).
    #[error("Invalid digits: {0}")]
    InvalidDigits(usize),

    /// Invalid step duration in seconds.
    #[error("Invalid step duration: {0} seconds")]
    InvalidStep(u64),

    /// Error building or parsing OTP auth URI.
    #[error("Invalid OTP URI: {0}")]
    InvalidUri(String),

    /// QR code rendering failure.
    #[error("QR code generation error: {0}")]
    QrCodeError(String),

    /// Time measurement or clock error.
    #[error("System time error: {0}")]
    TimeError(String),

    /// General cryptographic or hashing failure.
    #[error("Crypto error: {0}")]
    CryptoError(String),

    /// Verification failed for given token.
    #[error("Verification failed: token mismatch or expired")]
    VerificationFailed,

    /// Backup code mismatch or invalid format.
    #[error("Invalid backup code: {0}")]
    InvalidBackupCode(String),
}
