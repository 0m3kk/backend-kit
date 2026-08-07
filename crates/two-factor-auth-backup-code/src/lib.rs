use async_trait::async_trait;
use secret_store::{SecretPath, SecretStore, SecretValue, SetSecretOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
pub use two_factor_auth::*;

/// A set of generated backup codes (plain text for user display, and SHA-256 hashed for database/secret store persistence).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupCodeSet {
    /// Plain text backup codes (display only once during enrollment).
    pub plain_codes: Vec<String>,
    /// SHA-256 hashed backup codes (safe for store persistence).
    pub hashed_codes: Vec<String>,
}

/// Production implementation of 2FA / Backup Code authentication backed by `SecretStore`.
#[derive(Clone)]
pub struct BackupCodeTwoFactorAuth<S: SecretStore> {
    store: Arc<S>,
}

impl<S: SecretStore> BackupCodeTwoFactorAuth<S> {
    /// Create a new `BackupCodeTwoFactorAuth` with the given secret store.
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    /// Get reference to secret store.
    pub fn store(&self) -> &Arc<S> {
        &self.store
    }

    /// Hash a single backup code using SHA-256 after normalizing whitespace/dashes.
    pub fn hash_code(&self, code: &str) -> String {
        let normalized = BackupCode::normalize(code);
        let mut hasher = Sha256::new();
        hasher.update(normalized.as_bytes());
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// Generate a set of `count` cryptographically random backup codes.
    pub fn generate_codes(&self, count: usize) -> BackupCodeSet {
        let plain_codes = BackupCode::generate_set(count);
        let hashed_codes = plain_codes.iter().map(|c| self.hash_code(c)).collect();
        BackupCodeSet {
            plain_codes,
            hashed_codes,
        }
    }

    /// Verify a submitted code against a list of hashed active codes.
    pub fn verify_and_consume(
        &self,
        submitted_code: &str,
        hashed_codes: &[String],
    ) -> Result<Option<Vec<String>>, TwoFactorError> {
        let target_hash = self.hash_code(submitted_code);

        let mut remaining = Vec::with_capacity(hashed_codes.len());
        let mut found = false;

        for hash in hashed_codes {
            if !found && constant_time_compare(hash, &target_hash) {
                found = true;
            } else {
                remaining.push(hash.clone());
            }
        }

        if found { Ok(Some(remaining)) } else { Ok(None) }
    }

    /// Enroll a user by generating backup codes, storing the hashed codes in `SecretStore` under `2fa/backup/{user_id}`,
    /// and returning the `BackupCodeSet` (containing plain text codes for user display).
    pub async fn enroll_user(
        &self,
        user_id: &str,
        count: usize,
    ) -> Result<BackupCodeSet, TwoFactorError> {
        let set = self.generate_codes(count);
        let path = SecretPath::new(format!("2fa/backup/{user_id}"))
            .map_err(|e| TwoFactorError::InvalidSecret(e.to_string()))?;
        let value = SecretValue::from(set.hashed_codes.join(","));

        self.store
            .set(path, value, SetSecretOptions::default())
            .await
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;

        Ok(set)
    }

    /// Verify a submitted backup code against the user's active backup codes in `SecretStore`.
    /// If valid, consumes the code and updates `SecretStore` with remaining active hashes.
    pub async fn verify_and_consume_user_code(
        &self,
        user_id: &str,
        submitted_code: &str,
    ) -> Result<bool, TwoFactorError> {
        let path = SecretPath::new(format!("2fa/backup/{user_id}"))
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

        let value_str = entry
            .value
            .as_str()
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;

        let hashed_codes: Vec<String> = value_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let consumed_res = self.verify_and_consume(submitted_code, &hashed_codes)?;

        match consumed_res {
            Some(remaining) => {
                if remaining.is_empty() {
                    self.store
                        .delete(&path)
                        .await
                        .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;
                } else {
                    let new_value = SecretValue::from(remaining.join(","));
                    self.store
                        .set(path, new_value, SetSecretOptions::default())
                        .await
                        .map_err(|e| TwoFactorError::CryptoError(e.to_string()))?;
                }
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Revoke and delete a user's backup codes from `SecretStore`.
    pub async fn revoke_user(&self, user_id: &str) -> Result<bool, TwoFactorError> {
        let path = SecretPath::new(format!("2fa/backup/{user_id}"))
            .map_err(|e| TwoFactorError::InvalidSecret(e.to_string()))?;

        self.store
            .delete(&path)
            .await
            .map_err(|e| TwoFactorError::CryptoError(e.to_string()))
    }
}

#[async_trait]
impl<S: SecretStore> TwoFactorProvider for BackupCodeTwoFactorAuth<S> {
    fn method(&self) -> TwoFactorMethod {
        TwoFactorMethod::BackupCode
    }

    async fn issue_challenge(
        &self,
        user_identifier: &str,
    ) -> Result<TwoFactorChallenge, TwoFactorError> {
        let challenge_id = format!("backup_{user_identifier}");
        Ok(TwoFactorChallenge::new(
            challenge_id,
            TwoFactorMethod::BackupCode,
        ))
    }

    async fn verify_response(
        &self,
        user_identifier: &str,
        response: &TwoFactorResponse,
    ) -> Result<bool, TwoFactorError> {
        if response.method != TwoFactorMethod::BackupCode {
            return Ok(false);
        }

        self.verify_and_consume_user_code(user_identifier, &response.response_data)
            .await
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
