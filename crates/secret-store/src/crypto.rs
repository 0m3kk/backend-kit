use std::collections::HashMap;
use std::sync::Arc;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce as AesNonce};
use chacha20poly1305::{ChaCha20Poly1305, Nonce as ChaChaNonce};
use rand::Rng;

use crate::errors::SecretError;
use crate::types::{CipherAlgorithm, EncryptedPayload};

/// Interface for retrieving encryption keys by key identifier.
pub trait KeyProvider: Send + Sync {
    /// Retrieve the 32-byte master key corresponding to the `key_id`.
    fn get_key(&self, key_id: &str) -> Result<Vec<u8>, SecretError>;
}

/// Static in-memory `KeyProvider` holding master key mappings.
#[derive(Debug, Clone, Default)]
pub struct StaticKeyProvider {
    keys: HashMap<String, Vec<u8>>,
}

impl StaticKeyProvider {
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
        }
    }

    pub fn with_key(
        mut self,
        key_id: impl Into<String>,
        key_bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, SecretError> {
        let bytes = key_bytes.into();
        if bytes.len() != 32 {
            return Err(SecretError::KeyProviderError(format!(
                "Master key must be exactly 32 bytes, got {}",
                bytes.len()
            )));
        }
        self.keys.insert(key_id.into(), bytes);
        Ok(self)
    }

    pub fn add_key(
        &mut self,
        key_id: impl Into<String>,
        key_bytes: impl Into<Vec<u8>>,
    ) -> Result<(), SecretError> {
        let bytes = key_bytes.into();
        if bytes.len() != 32 {
            return Err(SecretError::KeyProviderError(format!(
                "Master key must be exactly 32 bytes, got {}",
                bytes.len()
            )));
        }
        self.keys.insert(key_id.into(), bytes);
        Ok(())
    }
}

impl KeyProvider for StaticKeyProvider {
    fn get_key(&self, key_id: &str) -> Result<Vec<u8>, SecretError> {
        self.keys
            .get(key_id)
            .cloned()
            .ok_or_else(|| SecretError::KeyProviderError(format!("Key ID '{key_id}' not found")))
    }
}

impl KeyProvider for Arc<dyn KeyProvider> {
    fn get_key(&self, key_id: &str) -> Result<Vec<u8>, SecretError> {
        self.as_ref().get_key(key_id)
    }
}

/// Cryptographic helper for encrypting and decrypting secret values using AEAD algorithms.
pub struct SecretCrypto;

impl SecretCrypto {
    /// Encrypt plaintext using the specified algorithm and 32-byte key.
    pub fn encrypt(
        cipher_algo: CipherAlgorithm,
        key_id: impl Into<String>,
        key_bytes: &[u8],
        plaintext: &[u8],
    ) -> Result<EncryptedPayload, SecretError> {
        if key_bytes.len() != 32 {
            return Err(SecretError::EncryptionError(format!(
                "Key length must be 32 bytes, got {}",
                key_bytes.len()
            )));
        }

        let key_id = key_id.into();
        let mut nonce = vec![0u8; 12];
        rand::thread_rng().fill(&mut nonce[..]);

        match cipher_algo {
            CipherAlgorithm::Aes256Gcm => {
                let cipher_key = aes_gcm::Key::<Aes256Gcm>::from_slice(key_bytes);
                let cipher = Aes256Gcm::new(cipher_key);
                let aes_nonce = AesNonce::from_slice(&nonce);

                let ciphertext = cipher
                    .encrypt(aes_nonce, plaintext)
                    .map_err(|e| SecretError::EncryptionError(e.to_string()))?;

                Ok(EncryptedPayload {
                    cipher: CipherAlgorithm::Aes256Gcm,
                    key_id,
                    nonce,
                    ciphertext,
                    tag: None,
                })
            }
            CipherAlgorithm::ChaCha20Poly1305 => {
                let cipher_key = chacha20poly1305::Key::from_slice(key_bytes);
                let cipher = ChaCha20Poly1305::new(cipher_key);
                let chacha_nonce = ChaChaNonce::from_slice(&nonce);

                let ciphertext = cipher
                    .encrypt(chacha_nonce, plaintext)
                    .map_err(|e| SecretError::EncryptionError(e.to_string()))?;

                Ok(EncryptedPayload {
                    cipher: CipherAlgorithm::ChaCha20Poly1305,
                    key_id,
                    nonce,
                    ciphertext,
                    tag: None,
                })
            }
        }
    }

    /// Decrypt payload using 32-byte key.
    pub fn decrypt(payload: &EncryptedPayload, key_bytes: &[u8]) -> Result<Vec<u8>, SecretError> {
        if key_bytes.len() != 32 {
            return Err(SecretError::DecryptionError(format!(
                "Key length must be 32 bytes, got {}",
                key_bytes.len()
            )));
        }

        if payload.nonce.len() != 12 {
            return Err(SecretError::DecryptionError(format!(
                "Invalid nonce length: expected 12, got {}",
                payload.nonce.len()
            )));
        }

        match payload.cipher {
            CipherAlgorithm::Aes256Gcm => {
                let cipher_key = aes_gcm::Key::<Aes256Gcm>::from_slice(key_bytes);
                let cipher = Aes256Gcm::new(cipher_key);
                let aes_nonce = AesNonce::from_slice(&payload.nonce);

                cipher
                    .decrypt(aes_nonce, payload.ciphertext.as_ref())
                    .map_err(|e| {
                        SecretError::DecryptionError(format!("AES-GCM decryption failed: {e}"))
                    })
            }
            CipherAlgorithm::ChaCha20Poly1305 => {
                let cipher_key = chacha20poly1305::Key::from_slice(key_bytes);
                let cipher = ChaCha20Poly1305::new(cipher_key);
                let chacha_nonce = ChaChaNonce::from_slice(&payload.nonce);

                cipher
                    .decrypt(chacha_nonce, payload.ciphertext.as_ref())
                    .map_err(|e| {
                        SecretError::DecryptionError(format!(
                            "ChaCha20Poly1305 decryption failed: {e}"
                        ))
                    })
            }
        }
    }
}
