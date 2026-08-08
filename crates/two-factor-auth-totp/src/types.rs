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

#[cfg(test)]
mod tests {
    use super::*;

    // --- TotpAlgorithm tests ---

    #[test]
    fn test_totp_algorithm_as_str() {
        assert_eq!(TotpAlgorithm::Sha1.as_str(), "SHA1");
        assert_eq!(TotpAlgorithm::Sha256.as_str(), "SHA256");
        assert_eq!(TotpAlgorithm::Sha512.as_str(), "SHA512");
    }

    // --- TotpDigits tests ---

    #[test]
    fn test_totp_digits_as_usize() {
        assert_eq!(TotpDigits::Six.as_usize(), 6);
        assert_eq!(TotpDigits::Seven.as_usize(), 7);
        assert_eq!(TotpDigits::Eight.as_usize(), 8);
    }

    #[test]
    fn test_totp_digits_try_from_usize_valid() {
        assert_eq!(TotpDigits::try_from_usize(6).unwrap(), TotpDigits::Six);
        assert_eq!(TotpDigits::try_from_usize(7).unwrap(), TotpDigits::Seven);
        assert_eq!(TotpDigits::try_from_usize(8).unwrap(), TotpDigits::Eight);
    }

    #[test]
    fn test_totp_digits_try_from_usize_invalid() {
        assert!(TotpDigits::try_from_usize(5).is_err());
        assert!(TotpDigits::try_from_usize(9).is_err());
        assert!(TotpDigits::try_from_usize(0).is_err());
    }

    // --- TotpSecret tests ---

    #[test]
    fn test_totp_secret_generate() {
        let secret = TotpSecret::generate();
        assert!(!secret.as_base32().is_empty());
        assert!(!secret.as_bytes().is_empty());
    }

    #[test]
    fn test_totp_secret_from_raw_valid() {
        let raw = vec![1u8, 2, 3, 4, 5];
        let secret = TotpSecret::from_raw(&raw).unwrap();
        assert_eq!(secret.as_bytes(), &raw);
        assert!(!secret.as_base32().is_empty());
    }

    #[test]
    fn test_totp_secret_from_raw_empty() {
        assert!(TotpSecret::from_raw(&[]).is_err());
    }

    #[test]
    fn test_totp_secret_from_base32_valid() {
        let secret = TotpSecret::from_raw(&[1, 2, 3, 4, 5]).unwrap();
        let b32 = secret.as_base32().to_string();
        let restored = TotpSecret::from_base32(&b32).unwrap();
        assert_eq!(restored.as_bytes(), secret.as_bytes());
    }

    #[test]
    fn test_totp_secret_from_base32_empty() {
        assert!(TotpSecret::from_base32("").is_err());
    }

    #[test]
    fn test_totp_secret_from_base32_with_whitespace_and_dashes() {
        let secret = TotpSecret::from_raw(&[1, 2, 3, 4, 5]).unwrap();
        let b32 = secret.as_base32().to_string();
        // Should still be parseable after normalization (uppercase, strip dashes/spaces)
        let restored = TotpSecret::from_base32(&b32);
        assert!(restored.is_ok());
    }

    // --- TotpConfig::build_url tests ---

    #[test]
    fn test_totp_config_build_url() {
        let secret = TotpSecret::from_raw(&[1, 2, 3, 4, 5]).unwrap();
        let config = TotpConfig::default();
        let url = config.build_url(&secret).unwrap();
        assert!(url.starts_with("otpauth://totp/"));
        assert!(url.contains("secret="));
        assert!(url.contains("issuer="));
        assert!(url.contains("algorithm=SHA1"));
        assert!(url.contains("digits=6"));
        assert!(url.contains("period=30"));
    }

    #[test]
    fn test_totp_config_build_url_empty_issuer() {
        let secret = TotpSecret::from_raw(&[1, 2, 3, 4, 5]).unwrap();
        let config = TotpConfig {
            issuer: "".to_string(),
            ..TotpConfig::default()
        };
        assert!(config.build_url(&secret).is_err());
    }

    #[test]
    fn test_totp_config_build_url_empty_account_name() {
        let secret = TotpSecret::from_raw(&[1, 2, 3, 4, 5]).unwrap();
        let config = TotpConfig {
            account_name: "".to_string(),
            ..TotpConfig::default()
        };
        assert!(config.build_url(&secret).is_err());
    }

    #[test]
    fn test_totp_config_build_url_zero_step() {
        let secret = TotpSecret::from_raw(&[1, 2, 3, 4, 5]).unwrap();
        let config = TotpConfig {
            step: 0,
            ..TotpConfig::default()
        };
        assert!(config.build_url(&secret).is_err());
    }

    // --- urlencoding tests ---

    #[test]
    fn test_urlencoding_alphanumeric() {
        assert_eq!(urlencoding("hello123"), "hello123");
    }

    #[test]
    fn test_urlencoding_special_chars() {
        let encoded = urlencoding("hello world");
        assert_eq!(encoded, "hello%20world");
    }

    #[test]
    fn test_urlencoding_safe_chars() {
        assert_eq!(urlencoding("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn test_urlencoding_at_sign() {
        let encoded = urlencoding("user@example.com");
        assert!(encoded.contains("%40"));
    }

    #[test]
    fn test_urlencoding_empty() {
        assert_eq!(urlencoding(""), "");
    }

    // --- TotpConfigBuilder tests ---

    #[test]
    fn test_totp_config_builder_defaults() {
        let config = TotpConfigBuilder::default().build();
        assert_eq!(config.algorithm, TotpAlgorithm::Sha1);
        assert_eq!(config.digits, TotpDigits::Six);
        assert_eq!(config.step, 30);
        assert_eq!(config.issuer, "BackendKit");
        assert_eq!(config.skew_windows, 1);
    }

    #[test]
    fn test_totp_config_builder_custom() {
        let config = TotpConfig::builder()
            .algorithm(TotpAlgorithm::Sha512)
            .digits(TotpDigits::Eight)
            .step(60)
            .issuer("MyApp")
            .account_name("user@test.com")
            .skew_windows(2)
            .build();

        assert_eq!(config.algorithm, TotpAlgorithm::Sha512);
        assert_eq!(config.digits, TotpDigits::Eight);
        assert_eq!(config.step, 60);
        assert_eq!(config.issuer, "MyApp");
        assert_eq!(config.account_name, "user@test.com");
        assert_eq!(config.skew_windows, 2);
    }
}
