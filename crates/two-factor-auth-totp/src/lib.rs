pub mod types;

use std::sync::Arc;
pub use two_factor_auth::*;
pub use types::{TotpAlgorithm, TotpConfig, TotpConfigBuilder, TotpDigits, TotpSecret};

use async_trait::async_trait;
use secret_store::{SecretPath, SecretStore, SecretValue, SetSecretOptions};
use totp_rs::{Algorithm as TotpRsAlgorithm, Secret as TotpRsSecret, Totp as TotpRs};

/// Production implementation of 2FA / TOTP authentication backed by `totp-rs` and `SecretStore`.
#[derive(Clone)]
pub struct TotpTwoFactorAuth<S: SecretStore> {
    store: Arc<S>,
    config: TotpConfig,
}

impl<S: SecretStore> TotpTwoFactorAuth<S> {
    /// Create a new `TotpTwoFactorAuth` with default configuration and given secret store.
    pub fn new(store: Arc<S>) -> Self {
        Self {
            store,
            config: TotpConfig::default(),
        }
    }

    /// Create a new `TotpTwoFactorAuth` with custom configuration and given secret store.
    pub fn with_config(store: Arc<S>, config: TotpConfig) -> Self {
        Self { store, config }
    }

    /// Get reference to secret store.
    pub fn store(&self) -> &Arc<S> {
        &self.store
    }

    /// Get current TOTP configuration.
    pub fn config(&self) -> &TotpConfig {
        &self.config
    }

    pub fn generate_secret(&self) -> Result<TotpSecret, TwoFactorError> {
        let secret_gen = TotpRsSecret::default();
        let raw = secret_gen.as_bytes();
        TotpSecret::from_raw(raw)
    }

    pub fn build_totp(
        &self,
        secret: &TotpSecret,
        skew_windows: u8,
    ) -> Result<TotpRs, TwoFactorError> {
        let algo = match self.config.algorithm {
            TotpAlgorithm::Sha1 => TotpRsAlgorithm::SHA1,
            TotpAlgorithm::Sha256 => TotpRsAlgorithm::SHA256,
            TotpAlgorithm::Sha512 => TotpRsAlgorithm::SHA512,
        };

        let secret_bytes = secret.as_bytes().to_vec();

        #[allow(deprecated)]
        TotpRs::new(
            algo,
            self.config.digits.as_usize(),
            skew_windows,
            self.config.step,
            secret_bytes,
            Some(self.config.issuer.clone()),
            self.config.account_name.clone(),
        )
        .map_err(|e| TwoFactorError::CryptoError(e.to_string()))
    }

    pub fn generate_token(
        &self,
        secret: &TotpSecret,
        timestamp: u64,
    ) -> Result<String, TwoFactorError> {
        let totp = self.build_totp(secret, self.config.skew_windows)?;
        Ok(totp.generate(timestamp).to_string())
    }

    pub fn verify_token(
        &self,
        secret: &TotpSecret,
        token: &str,
        timestamp: u64,
        skew_windows: u8,
    ) -> Result<bool, TwoFactorError> {
        let token = token.trim();
        if token.len() != self.config.digits.as_usize() {
            return Ok(false);
        }
        let totp = self.build_totp(secret, skew_windows)?;
        Ok(totp.check(token, timestamp).is_some())
    }

    pub fn build_otpauth_url(
        &self,
        secret: &TotpSecret,
        config: &TotpConfig,
    ) -> Result<String, TwoFactorError> {
        let temp_service = TotpTwoFactorAuth::with_config(self.store.clone(), config.clone());
        let totp = temp_service.build_totp(secret, config.skew_windows)?;
        #[allow(deprecated)]
        Ok(totp.get_url())
    }

    #[cfg(feature = "qr")]
    pub fn generate_qr_base64(&self, secret: &TotpSecret) -> Result<String, TwoFactorError> {
        let totp = self.build_totp(secret, self.config.skew_windows)?;
        #[allow(deprecated)]
        totp.get_qr_base64()
            .map_err(|e| TwoFactorError::QrCodeError(e.to_string()))
    }

    #[cfg(feature = "qr")]
    pub fn generate_qr_png(&self, secret: &TotpSecret) -> Result<Vec<u8>, TwoFactorError> {
        let totp = self.build_totp(secret, self.config.skew_windows)?;
        #[allow(deprecated)]
        totp.get_qr_png()
            .map_err(|e| TwoFactorError::QrCodeError(e.to_string()))
    }

    /// Enroll a user by generating a secret, storing it securely in `SecretStore` under `2fa/totp/{user_id}`,
    /// and returning the generated secret and setup URL.
    pub async fn enroll_user(&self, user_id: &str) -> Result<(TotpSecret, String), TwoFactorError> {
        let secret = self.generate_secret()?;
        let path = SecretPath::new(format!("2fa/totp/{user_id}"))
            .map_err(|e| TwoFactorError::InvalidSecret(e.to_string()))?;
        let value = SecretValue::from(secret.as_base32());

        self.store
            .set(path, value, SetSecretOptions::default())
            .await
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;

        let url = self.build_otpauth_url(&secret, &self.config)?;
        Ok((secret, url))
    }

    /// Retrieve user's secret from `SecretStore` and verify submitted TOTP token.
    pub async fn verify_user_token(
        &self,
        user_id: &str,
        token: &str,
        timestamp: u64,
    ) -> Result<bool, TwoFactorError> {
        let path = SecretPath::new(format!("2fa/totp/{user_id}"))
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

        let secret_str = entry
            .value
            .as_str()
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;
        let secret = TotpSecret::from_base32(secret_str)?;
        self.verify_token(&secret, token, timestamp, self.config.skew_windows)
    }

    /// Revoke and delete a user's TOTP secret from `SecretStore`.
    pub async fn revoke_user(&self, user_id: &str) -> Result<bool, TwoFactorError> {
        let path = SecretPath::new(format!("2fa/totp/{user_id}"))
            .map_err(|e| TwoFactorError::InvalidSecret(e.to_string()))?;

        self.store
            .delete(&path)
            .await
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))
    }
}

#[async_trait]
impl<S: SecretStore> TwoFactorProvider for TotpTwoFactorAuth<S> {
    fn method(&self) -> TwoFactorMethod {
        TwoFactorMethod::Totp
    }

    async fn issue_challenge(
        &self,
        user_identifier: &str,
    ) -> Result<TwoFactorChallenge, TwoFactorError> {
        let (_secret, url) = self.enroll_user(user_identifier).await?;

        let challenge_id = format!("totp_{user_identifier}");
        Ok(TwoFactorChallenge::new(challenge_id, TwoFactorMethod::Totp).with_payload(url))
    }

    async fn verify_response(
        &self,
        user_identifier: &str,
        response: &TwoFactorResponse,
    ) -> Result<bool, TwoFactorError> {
        if response.method != TwoFactorMethod::Totp {
            return Ok(false);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| TwoFactorError::TimeError(e.to_string()))?
            .as_secs();

        self.verify_user_token(user_identifier, &response.response_data, now)
            .await
    }
}
