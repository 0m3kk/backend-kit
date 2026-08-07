use rand::RngExt;
use serde::{Deserialize, Serialize};
use two_factor_auth::errors::TwoFactorError;

/// Supported HMAC hashing algorithms for TOTP (RFC 6238).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TotpAlgorithm {
    #[default]
    Sha1,
    Sha256,
    Sha512,
}

impl TotpAlgorithm {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sha1 => "SHA1",
            Self::Sha256 => "SHA256",
            Self::Sha512 => "SHA512",
        }
    }
}

/// Number of digits in generated TOTP token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TotpDigits {
    #[default]
    Six = 6,
    Seven = 7,
    Eight = 8,
}

impl TotpDigits {
    pub fn as_usize(&self) -> usize {
        *self as usize
    }

    pub fn try_from_usize(val: usize) -> Result<Self, TwoFactorError> {
        match val {
            6 => Ok(Self::Six),
            7 => Ok(Self::Seven),
            8 => Ok(Self::Eight),
            _ => Err(TwoFactorError::InvalidDigits(val)),
        }
    }
}

/// TOTP Secret Key container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TotpSecret {
    base32: String,
    raw: Vec<u8>,
}

impl TotpSecret {
    pub fn generate() -> Self {
        let mut bytes = [0u8; 20];
        rand::rng().fill(&mut bytes);
        let b32 = base32::encode(base32::Alphabet::Rfc4648 { padding: false }, &bytes);
        Self {
            base32: b32,
            raw: bytes.to_vec(),
        }
    }

    pub fn from_base32(b32: &str) -> Result<Self, TwoFactorError> {
        let clean = b32.trim().to_uppercase().replace([' ', '-'], "");
        if clean.is_empty() {
            return Err(TwoFactorError::InvalidSecret(
                "Secret string cannot be empty".to_string(),
            ));
        }

        let raw = base32::decode(base32::Alphabet::Rfc4648 { padding: false }, &clean)
            .or_else(|| base32::decode(base32::Alphabet::Rfc4648 { padding: true }, &clean))
            .ok_or_else(|| {
                TwoFactorError::InvalidSecret("Invalid base32 secret encoding".to_string())
            })?;

        Ok(Self { base32: clean, raw })
    }

    pub fn from_raw(raw: &[u8]) -> Result<Self, TwoFactorError> {
        if raw.is_empty() {
            return Err(TwoFactorError::InvalidSecret(
                "Raw secret bytes cannot be empty".to_string(),
            ));
        }
        let b32 = base32::encode(base32::Alphabet::Rfc4648 { padding: false }, raw);
        Ok(Self {
            base32: b32,
            raw: raw.to_vec(),
        })
    }

    pub fn as_base32(&self) -> &str {
        &self.base32
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.raw
    }
}

/// Configuration settings for TOTP generation and verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TotpConfig {
    pub algorithm: TotpAlgorithm,
    pub digits: TotpDigits,
    pub step: u64,
    pub issuer: String,
    pub account_name: String,
    pub skew_windows: u8,
}

impl Default for TotpConfig {
    fn default() -> Self {
        Self {
            algorithm: TotpAlgorithm::Sha1,
            digits: TotpDigits::Six,
            step: 30,
            issuer: "BackendKit".to_string(),
            account_name: "user@example.com".to_string(),
            skew_windows: 1,
        }
    }
}

impl TotpConfig {
    pub fn builder() -> TotpConfigBuilder {
        TotpConfigBuilder::default()
    }

    pub fn build_url(&self, secret: &TotpSecret) -> Result<String, TwoFactorError> {
        if self.issuer.is_empty() {
            return Err(TwoFactorError::InvalidUri(
                "Issuer cannot be empty".to_string(),
            ));
        }
        if self.account_name.is_empty() {
            return Err(TwoFactorError::InvalidUri(
                "Account name cannot be empty".to_string(),
            ));
        }
        if self.step == 0 {
            return Err(TwoFactorError::InvalidStep(0));
        }

        let encoded_issuer = urlencoding(&self.issuer);
        let encoded_account = urlencoding(&self.account_name);

        let url = format!(
            "otpauth://totp/{encoded_issuer}:{encoded_account}?secret={}&issuer={encoded_issuer}&algorithm={}&digits={}&period={}",
            secret.as_base32(),
            self.algorithm.as_str(),
            self.digits.as_usize(),
            self.step,
        );

        Ok(url)
    }
}

#[derive(Debug, Clone, Default)]
pub struct TotpConfigBuilder {
    algorithm: Option<TotpAlgorithm>,
    digits: Option<TotpDigits>,
    step: Option<u64>,
    issuer: Option<String>,
    account_name: Option<String>,
    skew_windows: Option<u8>,
}

impl TotpConfigBuilder {
    pub fn algorithm(mut self, algo: TotpAlgorithm) -> Self {
        self.algorithm = Some(algo);
        self
    }

    pub fn digits(mut self, digits: TotpDigits) -> Self {
        self.digits = Some(digits);
        self
    }

    pub fn step(mut self, step: u64) -> Self {
        self.step = Some(step);
        self
    }

    pub fn issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = Some(issuer.into());
        self
    }

    pub fn account_name(mut self, account_name: impl Into<String>) -> Self {
        self.account_name = Some(account_name.into());
        self
    }

    pub fn skew_windows(mut self, skew: u8) -> Self {
        self.skew_windows = Some(skew);
        self
    }

    pub fn build(self) -> TotpConfig {
        TotpConfig {
            algorithm: self.algorithm.unwrap_or_default(),
            digits: self.digits.unwrap_or_default(),
            step: self.step.unwrap_or(30),
            issuer: self.issuer.unwrap_or_else(|| "BackendKit".to_string()),
            account_name: self
                .account_name
                .unwrap_or_else(|| "user@example.com".to_string()),
            skew_windows: self.skew_windows.unwrap_or(1),
        }
    }
}

fn urlencoding(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u32),
        })
        .collect()
}
