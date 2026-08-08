use async_trait::async_trait;
pub use email_sender::{Email, EmailAddress, EmailError, EmailSender};
use rand::RngExt;
use secret_store::{SecretPath, SecretStore, SecretValue, SetSecretOptions};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
pub use two_factor_auth::*;

/// Configuration options for Email OTP passcode generation and template rendering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailOtpConfig {
    /// Number of numeric digits in the Email OTP code (default: 6).
    pub code_length: usize,
    /// Time-to-live duration before the OTP expires (default: 600 seconds / 10 minutes).
    pub ttl: std::time::Duration,
    /// Email subject line.
    pub subject: String,
    /// HTML body template with `{}` placeholder for the OTP code.
    pub html_template: String,
    /// Plain text body template with `{}` placeholder for the OTP code.
    pub text_template: String,
}

impl Default for EmailOtpConfig {
    fn default() -> Self {
        Self {
            code_length: 6,
            ttl: std::time::Duration::from_secs(600),
            subject: "Your Verification Code".to_string(),
            html_template: "<p>Your 2FA verification code is: <strong>{}</strong></p>".to_string(),
            text_template: "Your 2FA verification code is: {}".to_string(),
        }
    }
}

/// Production implementation of Email 2FA backed by `SecretStore` and pluggable `EmailSender`.
#[derive(Clone)]
pub struct EmailTwoFactorAuth<S: SecretStore, E: EmailSender> {
    store: Arc<S>,
    email_sender: Arc<E>,
    sender_address: EmailAddress,
    config: EmailOtpConfig,
}

impl<S: SecretStore, E: EmailSender> EmailTwoFactorAuth<S, E> {
    /// Create a new `EmailTwoFactorAuth` instance with default configuration.
    pub fn new(store: Arc<S>, email_sender: Arc<E>, sender_address: EmailAddress) -> Self {
        Self {
            store,
            email_sender,
            sender_address,
            config: EmailOtpConfig::default(),
        }
    }

    /// Create a new `EmailTwoFactorAuth` instance with custom configuration.
    pub fn with_config(
        store: Arc<S>,
        email_sender: Arc<E>,
        sender_address: EmailAddress,
        config: EmailOtpConfig,
    ) -> Self {
        Self {
            store,
            email_sender,
            sender_address,
            config,
        }
    }

    /// Reference to secret store.
    pub fn store(&self) -> &Arc<S> {
        &self.store
    }

    /// Reference to email sender.
    pub fn email_sender(&self) -> &Arc<E> {
        &self.email_sender
    }

    /// Default sender email address.
    pub fn sender_address(&self) -> &EmailAddress {
        &self.sender_address
    }

    /// Current Email OTP config.
    pub fn config(&self) -> &EmailOtpConfig {
        &self.config
    }

    /// Enroll a user's email address for Email 2FA.
    pub async fn enroll_user(
        &self,
        user_id: &str,
        email_address: &str,
    ) -> Result<(), TwoFactorError> {
        let addr = EmailAddress::new(email_address.trim())
            .map_err(|e| TwoFactorError::InvalidSecret(e.to_string()))?;

        let path = SecretPath::new(format!("2fa/email_addr/{user_id}"))
            .map_err(|e| TwoFactorError::InvalidSecret(e.to_string()))?;
        let value = SecretValue::from(addr.email.as_str());

        self.store
            .set(path, value, SetSecretOptions::default())
            .await
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;

        Ok(())
    }

    /// Get user's enrolled email address.
    pub async fn get_user_email(&self, user_id: &str) -> Result<Option<String>, TwoFactorError> {
        let path = SecretPath::new(format!("2fa/email_addr/{user_id}"))
            .map_err(|e| TwoFactorError::InvalidSecret(e.to_string()))?;

        let entry = self
            .store
            .get(&path)
            .await
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;

        match entry {
            Some(e) => {
                let email = e
                    .value
                    .as_str()
                    .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;
                Ok(Some(email.to_string()))
            }
            None => Ok(None),
        }
    }

    /// Generate and send a new Email OTP code to an enrolled user.
    pub async fn send_code(&self, user_id: &str) -> Result<String, TwoFactorError> {
        let email_str = self.get_user_email(user_id).await?.ok_or_else(|| {
            TwoFactorError::InvalidSecret(format!("User {user_id} has no enrolled email address"))
        })?;

        self.send_code_to_email(user_id, &email_str).await
    }

    /// Generate and send a new Email OTP code directly to a specific email address.
    pub async fn send_code_to_email(
        &self,
        user_id: &str,
        email_str: &str,
    ) -> Result<String, TwoFactorError> {
        let recipient_addr = EmailAddress::new(email_str)
            .map_err(|e| TwoFactorError::InvalidSecret(e.to_string()))?;

        let code = generate_numeric_code(self.config.code_length);

        let path = SecretPath::new(format!("2fa/email_code/{user_id}"))
            .map_err(|e| TwoFactorError::InvalidSecret(e.to_string()))?;
        let value = SecretValue::from(code.as_str());

        let options = SetSecretOptions::default().with_ttl(self.config.ttl);

        self.store
            .set(path, value, options)
            .await
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;

        let html_body = self.config.html_template.replace("{}", &code);
        let text_body = self.config.text_template.replace("{}", &code);

        let email = Email::builder()
            .from(self.sender_address.clone())
            .to(recipient_addr)
            .subject(&self.config.subject)
            .html_body(html_body)
            .text_body(text_body)
            .build()
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;

        self.email_sender
            .send(&email)
            .await
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;

        Ok(mask_email(email_str))
    }

    /// Verify a submitted Email OTP code. Consumes the stored code on successful match.
    pub async fn verify_code(
        &self,
        user_id: &str,
        submitted_code: &str,
    ) -> Result<bool, TwoFactorError> {
        let path = SecretPath::new(format!("2fa/email_code/{user_id}"))
            .map_err(|e| TwoFactorError::InvalidSecret(e.to_string()))?;

        let entry = self
            .store
            .get(&path)
            .await
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;

        let entry = match entry {
            Some(e) => e,
            None => return Ok(false),
        };

        let stored_code = entry
            .value
            .as_str()
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;

        let is_valid = constant_time_compare(stored_code.trim(), submitted_code.trim());

        if is_valid {
            self.store
                .delete(&path)
                .await
                .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;
        }

        Ok(is_valid)
    }

    /// Revoke email enrollment and any pending Email OTP code for a user.
    pub async fn revoke_user(&self, user_id: &str) -> Result<bool, TwoFactorError> {
        let email_path = SecretPath::new(format!("2fa/email_addr/{user_id}"))
            .map_err(|e| TwoFactorError::InvalidSecret(e.to_string()))?;
        let code_path = SecretPath::new(format!("2fa/email_code/{user_id}"))
            .map_err(|e| TwoFactorError::InvalidSecret(e.to_string()))?;

        let res1 = self
            .store
            .delete(&email_path)
            .await
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;
        let _ = self.store.delete(&code_path).await;

        Ok(res1)
    }
}

#[async_trait]
impl<S: SecretStore, E: EmailSender> TwoFactorProvider for EmailTwoFactorAuth<S, E> {
    fn method(&self) -> TwoFactorMethod {
        TwoFactorMethod::EmailOtp
    }

    async fn issue_challenge(
        &self,
        user_identifier: &str,
    ) -> Result<TwoFactorChallenge, TwoFactorError> {
        let masked = self.send_code(user_identifier).await?;
        let challenge_id = format!("email_{user_identifier}");

        Ok(
            TwoFactorChallenge::new(challenge_id, TwoFactorMethod::EmailOtp)
                .with_payload(format!("Passcode sent to {masked}")),
        )
    }

    async fn verify_response(
        &self,
        user_identifier: &str,
        response: &TwoFactorResponse,
    ) -> Result<bool, TwoFactorError> {
        if response.method != TwoFactorMethod::EmailOtp {
            return Ok(false);
        }

        self.verify_code(user_identifier, &response.response_data)
            .await
    }
}

fn generate_numeric_code(length: usize) -> String {
    let mut rng = rand::rng();
    (0..length)
        .map(|_| rng.random_range(0..=9).to_string())
        .collect()
}

fn mask_email(email: &str) -> String {
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return "***@***".to_string();
    }
    let name = parts[0];
    let domain = parts[1];

    if name.len() <= 2 {
        format!("{}***@{}", &name[..1], domain)
    } else {
        format!("{}{}***@{}", &name[..1], &name[name.len() - 1..], domain)
    }
}

fn constant_time_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_email_standard() {
        let masked = mask_email("alice@example.com");
        assert_eq!(masked, "ae***@example.com");
    }

    #[test]
    fn test_mask_email_short_name() {
        let masked = mask_email("ab@example.com");
        assert_eq!(masked, "a***@example.com");
    }

    #[test]
    fn test_mask_email_single_char_name() {
        let masked = mask_email("a@example.com");
        assert_eq!(masked, "a***@example.com");
    }

    #[test]
    fn test_mask_email_invalid_no_at() {
        let masked = mask_email("no-at-sign");
        assert_eq!(masked, "***@***");
    }

    #[test]
    fn test_generate_numeric_code_length() {
        let code = generate_numeric_code(6);
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));

        let code8 = generate_numeric_code(8);
        assert_eq!(code8.len(), 8);
        assert!(code8.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_generate_numeric_code_zero_length() {
        let code = generate_numeric_code(0);
        assert!(code.is_empty());
    }

    #[test]
    fn test_constant_time_compare_equal() {
        assert!(constant_time_compare("abc123", "abc123"));
    }

    #[test]
    fn test_constant_time_compare_not_equal() {
        assert!(!constant_time_compare("abc123", "abc124"));
    }

    #[test]
    fn test_constant_time_compare_different_lengths() {
        assert!(!constant_time_compare("short", "longer_string"));
    }

    #[test]
    fn test_constant_time_compare_empty() {
        assert!(constant_time_compare("", ""));
    }
}
