use rand::Rng;
use serde::{Deserialize, Serialize};

/// Supported Two-Factor Authentication mechanisms.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TwoFactorMethod {
    /// Time-based One-Time Password (RFC 6238)
    Totp,
    /// SMS One-Time Password
    SmsOtp,
    /// Email One-Time Password
    EmailOtp,
    /// Single-use recovery / backup codes
    BackupCode,
}

impl std::fmt::Display for TwoFactorMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Totp => write!(f, "TOTP"),
            Self::SmsOtp => write!(f, "SMS_OTP"),
            Self::EmailOtp => write!(f, "EMAIL_OTP"),
            Self::BackupCode => write!(f, "BACKUP_CODE"),
        }
    }
}

/// Generic 2FA challenge issued to a user during enrollment or authentication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwoFactorChallenge {
    pub challenge_id: String,
    pub method: TwoFactorMethod,
    pub payload: Option<String>,
    pub expires_at: Option<u64>,
}

impl TwoFactorChallenge {
    pub fn new(challenge_id: impl Into<String>, method: TwoFactorMethod) -> Self {
        Self {
            challenge_id: challenge_id.into(),
            method,
            payload: None,
            expires_at: None,
        }
    }

    pub fn with_payload(mut self, payload: impl Into<String>) -> Self {
        self.payload = Some(payload.into());
        self
    }

    pub fn with_expiration(mut self, expires_at: u64) -> Self {
        self.expires_at = Some(expires_at);
        self
    }
}

/// Generic 2FA response / credential submitted by a user for verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwoFactorResponse {
    pub method: TwoFactorMethod,
    pub response_data: String,
}

impl TwoFactorResponse {
    pub fn new(method: TwoFactorMethod, response_data: impl Into<String>) -> Self {
        Self {
            method,
            response_data: response_data.into(),
        }
    }

    pub fn totp(code: impl Into<String>) -> Self {
        Self::new(TwoFactorMethod::Totp, code)
    }

    pub fn sms_otp(code: impl Into<String>) -> Self {
        Self::new(TwoFactorMethod::SmsOtp, code)
    }

    pub fn email_otp(code: impl Into<String>) -> Self {
        Self::new(TwoFactorMethod::EmailOtp, code)
    }

    pub fn backup_code(code: impl Into<String>) -> Self {
        Self::new(TwoFactorMethod::BackupCode, code)
    }
}

// --- Backup Code Primitives ---

/// Helper for generating formatted backup/recovery codes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupCode;

impl BackupCode {
    pub fn generate_set(count: usize) -> Vec<String> {
        let mut rng = rand::thread_rng();
        const CHARS: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";

        (0..count)
            .map(|_| {
                let code: String = (0..8)
                    .map(|_| {
                        let idx = rng.gen_range(0..=CHARS.len() - 1);
                        CHARS[idx] as char
                    })
                    .collect();
                format!("{}-{}", &code[0..4], &code[4..8])
            })
            .collect()
    }

    pub fn normalize(code: &str) -> String {
        code.trim().to_uppercase().replace([' ', '-'], "")
    }
}
