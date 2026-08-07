use async_trait::async_trait;
use rand::RngExt;
use secret_store::{SecretPath, SecretStore, SecretValue, SetSecretOptions};
use serde::{Deserialize, Serialize};
pub use sms_sender::{SmsError, SmsMessage, SmsSender};
use std::sync::Arc;
pub use two_factor_auth::*;

/// Configuration options for SMS OTP passcode generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmsOtpConfig {
    /// Number of numeric digits in the SMS OTP code (default: 6).
    pub code_length: usize,
    /// Time-to-live duration before the OTP expires (default: 300 seconds / 5 minutes).
    pub ttl: std::time::Duration,
    /// SMS message template with `{}` placeholder for the OTP code.
    pub message_template: String,
}

impl Default for SmsOtpConfig {
    fn default() -> Self {
        Self {
            code_length: 6,
            ttl: std::time::Duration::from_secs(300),
            message_template: "Your verification code is: {}".to_string(),
        }
    }
}

/// Production implementation of SMS 2FA backed by `SecretStore` and pluggable `SmsSender`.
#[derive(Clone)]
pub struct SmsTwoFactorAuth<S: SecretStore, P: SmsSender> {
    store: Arc<S>,
    sms_sender: Arc<P>,
    config: SmsOtpConfig,
}

impl<S: SecretStore, P: SmsSender> SmsTwoFactorAuth<S, P> {
    /// Create a new `SmsTwoFactorAuth` instance with default configuration.
    pub fn new(store: Arc<S>, sms_sender: Arc<P>) -> Self {
        Self {
            store,
            sms_sender,
            config: SmsOtpConfig::default(),
        }
    }

    /// Create a new `SmsTwoFactorAuth` instance with custom configuration.
    pub fn with_config(store: Arc<S>, sms_sender: Arc<P>, config: SmsOtpConfig) -> Self {
        Self {
            store,
            sms_sender,
            config,
        }
    }

    /// Reference to secret store.
    pub fn store(&self) -> &Arc<S> {
        &self.store
    }

    /// Reference to SMS sender.
    pub fn sms_sender(&self) -> &Arc<P> {
        &self.sms_sender
    }

    /// Current SMS OTP config.
    pub fn config(&self) -> &SmsOtpConfig {
        &self.config
    }

    /// Enroll a user's phone number for SMS 2FA.
    pub async fn enroll_user(
        &self,
        user_id: &str,
        phone_number: &str,
    ) -> Result<(), TwoFactorError> {
        let path = SecretPath::new(format!("2fa/sms_phone/{user_id}"))
            .map_err(|e| TwoFactorError::InvalidSecret(e.to_string()))?;
        let value = SecretValue::from(phone_number.trim());

        self.store
            .set(path, value, SetSecretOptions::default())
            .await
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;

        Ok(())
    }

    /// Get user's enrolled phone number.
    pub async fn get_user_phone(&self, user_id: &str) -> Result<Option<String>, TwoFactorError> {
        let path = SecretPath::new(format!("2fa/sms_phone/{user_id}"))
            .map_err(|e| TwoFactorError::InvalidSecret(e.to_string()))?;

        let entry = self
            .store
            .get(&path)
            .await
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;

        match entry {
            Some(e) => {
                let phone = e
                    .value
                    .as_str()
                    .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;
                Ok(Some(phone.to_string()))
            }
            None => Ok(None),
        }
    }

    /// Generate and send a new SMS OTP code to an enrolled user.
    pub async fn send_code(&self, user_id: &str) -> Result<String, TwoFactorError> {
        let phone = self.get_user_phone(user_id).await?.ok_or_else(|| {
            TwoFactorError::InvalidSecret(format!("User {user_id} has no enrolled phone number"))
        })?;

        self.send_code_to_phone(user_id, &phone).await
    }

    /// Generate and send a new SMS OTP code directly to a specific phone number.
    pub async fn send_code_to_phone(
        &self,
        user_id: &str,
        phone_number: &str,
    ) -> Result<String, TwoFactorError> {
        let code = generate_numeric_code(self.config.code_length);

        let path = SecretPath::new(format!("2fa/sms_code/{user_id}"))
            .map_err(|e| TwoFactorError::InvalidSecret(e.to_string()))?;
        let value = SecretValue::from(code.as_str());

        let options = SetSecretOptions::default().with_ttl(self.config.ttl);

        self.store
            .set(path, value, options)
            .await
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;

        let body = self.config.message_template.replace("{}", &code);
        let msg = SmsMessage::new(phone_number, body);

        self.sms_sender
            .send_sms(&msg)
            .await
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;

        Ok(mask_phone(phone_number))
    }

    /// Verify a submitted SMS OTP code. Consumes the stored code on successful match.
    pub async fn verify_code(
        &self,
        user_id: &str,
        submitted_code: &str,
    ) -> Result<bool, TwoFactorError> {
        let path = SecretPath::new(format!("2fa/sms_code/{user_id}"))
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

    /// Revoke phone enrollment and any pending SMS OTP code for a user.
    pub async fn revoke_user(&self, user_id: &str) -> Result<bool, TwoFactorError> {
        let phone_path = SecretPath::new(format!("2fa/sms_phone/{user_id}"))
            .map_err(|e| TwoFactorError::InvalidSecret(e.to_string()))?;
        let code_path = SecretPath::new(format!("2fa/sms_code/{user_id}"))
            .map_err(|e| TwoFactorError::InvalidSecret(e.to_string()))?;

        let res1 = self
            .store
            .delete(&phone_path)
            .await
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;
        let _ = self.store.delete(&code_path).await;

        Ok(res1)
    }
}

#[async_trait]
impl<S: SecretStore, P: SmsSender> TwoFactorProvider for SmsTwoFactorAuth<S, P> {
    fn method(&self) -> TwoFactorMethod {
        TwoFactorMethod::SmsOtp
    }

    async fn issue_challenge(
        &self,
        user_identifier: &str,
    ) -> Result<TwoFactorChallenge, TwoFactorError> {
        let masked = self.send_code(user_identifier).await?;
        let challenge_id = format!("sms_{user_identifier}");

        Ok(
            TwoFactorChallenge::new(challenge_id, TwoFactorMethod::SmsOtp)
                .with_payload(format!("Passcode sent to {masked}")),
        )
    }

    async fn verify_response(
        &self,
        user_identifier: &str,
        response: &TwoFactorResponse,
    ) -> Result<bool, TwoFactorError> {
        if response.method != TwoFactorMethod::SmsOtp {
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

fn mask_phone(phone: &str) -> String {
    let trimmed = phone.trim();
    if trimmed.len() <= 4 {
        return "****".to_string();
    }
    let suffix = &trimmed[trimmed.len() - 4..];
    format!("+*****{suffix}")
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
