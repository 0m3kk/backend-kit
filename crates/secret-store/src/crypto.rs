use std::collections::BTreeMap;
use std::fmt;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce as AesNonce};
use chacha20poly1305::{ChaCha20Poly1305, Nonce as ChaChaNonce};
use rand::Rng;

use crate::errors::SecretError;
use crate::types::{CipherAlgorithm, EncryptedPayload};

/// Length in bytes of master keys (KEKs) and data encryption keys (DEKs).
pub const KEY_LEN: usize = 32;

/// A versioned master key (KEK) used to wrap DEKs in envelope encryption.
#[derive(Clone)]
pub struct MasterKey {
    version: u32,
    key: [u8; KEY_LEN],
}

impl MasterKey {
    /// Creates a new `MasterKey` for the given version and 32-byte key material.
    pub fn new(version: u32, key: [u8; KEY_LEN]) -> Self {
        Self { version, key }
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn key(&self) -> &[u8; KEY_LEN] {
        &self.key
    }
}

impl fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MasterKey")
            .field("version", &self.version)
            .field("key", &"[redacted]")
            .finish()
    }
}

/// A set of versioned master keys (KEKs) for envelope encryption.
///
/// The highest version is automatically selected as the active key for wrapping DEKs on writes.
/// Older versions remain available for unwrapping existing DEKs during reads and rotation.
#[derive(Debug, Clone)]
pub struct KeyRing {
    keys: BTreeMap<u32, MasterKey>,
    current_version: u32,
}

impl KeyRing {
    /// Builds a `KeyRing` from one or more master keys. The highest version becomes the current key.
    pub fn new(keys: impl IntoIterator<Item = MasterKey>) -> Result<Self, SecretError> {
        let mut ring = BTreeMap::new();
        for key in keys {
            let version = key.version();
            if ring.insert(version, key).is_some() {
                return Err(SecretError::InvalidKey(format!(
                    "Duplicate master key version {version}"
                )));
            }
        }
        let current_version = ring.keys().next_back().copied().ok_or_else(|| {
            SecretError::InvalidKey("Key ring must contain at least one master key".to_string())
        })?;

        Ok(Self {
            keys: ring,
            current_version,
        })
    }

    /// Retrieve the current master key version used for writes.
    pub fn current_version(&self) -> u32 {
        self.current_version
    }

    /// Retrieve a reference to a `MasterKey` by version.
    pub fn get_key(&self, version: u32) -> Result<&MasterKey, SecretError> {
        self.keys
            .get(&version)
            .ok_or(SecretError::UnknownKeyVersion(version))
    }

    /// Wrap (encrypt) a 32-byte DEK under the current master key.
    pub fn wrap_dek(&self, dek: &[u8; KEY_LEN]) -> Result<(Vec<u8>, u32), SecretError> {
        let kek = self
            .keys
            .get(&self.current_version)
            .ok_or_else(|| SecretError::InvalidKey("Key ring is empty".to_string()))?;

        let mut nonce = vec![0u8; 12];
        rand::thread_rng().fill(&mut nonce[..]);

        let cipher_key = aes_gcm::Key::<Aes256Gcm>::from(kek.key);
        let cipher = Aes256Gcm::new(&cipher_key);
        let aes_nonce = AesNonce::try_from(nonce.as_slice())
            .map_err(|e| SecretError::EncryptionError(e.to_string()))?;

        let mut wrapped = cipher
            .encrypt(&aes_nonce, dek.as_ref())
            .map_err(|e| SecretError::EncryptionError(format!("DEK wrap failed: {e}")))?;

        let mut blob = Vec::with_capacity(12 + wrapped.len());
        blob.extend_from_slice(&nonce);
        blob.append(&mut wrapped);

        Ok((blob, self.current_version))
    }

    /// Unwrap (decrypt) a DEK previously wrapped under `kek_version`.
    pub fn unwrap_dek(
        &self,
        kek_version: u32,
        wrapped: &[u8],
    ) -> Result<[u8; KEY_LEN], SecretError> {
        let kek = self.get_key(kek_version)?;

        if wrapped.len() < 12 {
            return Err(SecretError::DecryptionError(
                "Truncated wrapped DEK blob".to_string(),
            ));
        }

        let (nonce, ciphertext) = wrapped.split_at(12);
        let cipher_key = aes_gcm::Key::<Aes256Gcm>::from(kek.key);
        let cipher = Aes256Gcm::new(&cipher_key);
        let aes_nonce =
            AesNonce::try_from(nonce).map_err(|e| SecretError::DecryptionError(e.to_string()))?;

        let dek_bytes = cipher
            .decrypt(&aes_nonce, ciphertext)
            .map_err(|e| SecretError::DecryptionError(format!("DEK unwrap failed: {e}")))?;

        let dek: [u8; KEY_LEN] = dek_bytes
            .try_into()
            .map_err(|_| SecretError::DecryptionError("Invalid DEK length".to_string()))?;

        Ok(dek)
    }
}

/// Generates a fresh 32-byte random Data Encryption Key (DEK).
pub fn generate_dek() -> Result<[u8; KEY_LEN], SecretError> {
    let mut dek = [0u8; KEY_LEN];
    rand::thread_rng().fill(&mut dek);
    Ok(dek)
}

/// Cryptographic helper performing envelope encryption and decryption of secrets.
pub struct SecretCrypto;

impl SecretCrypto {
    /// Encrypt plaintext secret payload using envelope encryption (DEK + KeyRing KEK).
    pub fn encrypt_envelope(
        cipher_algo: CipherAlgorithm,
        keyring: &KeyRing,
        plaintext: &[u8],
    ) -> Result<EncryptedPayload, SecretError> {
        let dek = generate_dek()?;
        let (wrapped_dek, kek_version) = keyring.wrap_dek(&dek)?;

        let mut nonce = vec![0u8; 12];
        rand::thread_rng().fill(&mut nonce[..]);

        let ciphertext = match cipher_algo {
            CipherAlgorithm::Aes256Gcm => {
                let cipher_key = aes_gcm::Key::<Aes256Gcm>::from(dek);
                let cipher = Aes256Gcm::new(&cipher_key);
                let aes_nonce = AesNonce::try_from(nonce.as_slice())
                    .map_err(|e| SecretError::EncryptionError(e.to_string()))?;
                cipher
                    .encrypt(&aes_nonce, plaintext)
                    .map_err(|e| SecretError::EncryptionError(e.to_string()))?
            }
            CipherAlgorithm::ChaCha20Poly1305 => {
                let cipher_key = chacha20poly1305::Key::from(dek);
                let cipher = ChaCha20Poly1305::new(&cipher_key);
                let chacha_nonce = ChaChaNonce::try_from(nonce.as_slice())
                    .map_err(|e| SecretError::EncryptionError(e.to_string()))?;
                cipher
                    .encrypt(&chacha_nonce, plaintext)
                    .map_err(|e| SecretError::EncryptionError(e.to_string()))?
            }
        };

        Ok(EncryptedPayload {
            cipher: cipher_algo,
            kek_version,
            wrapped_dek,
            nonce,
            ciphertext,
            tag: None,
        })
    }

    /// Decrypt secret payload by unwrapping DEK from `KeyRing` and decrypting ciphertext.
    pub fn decrypt_envelope(
        payload: &EncryptedPayload,
        keyring: &KeyRing,
    ) -> Result<Vec<u8>, SecretError> {
        let dek = keyring.unwrap_dek(payload.kek_version, &payload.wrapped_dek)?;

        if payload.nonce.len() != 12 {
            return Err(SecretError::DecryptionError(format!(
                "Invalid nonce length: expected 12, got {}",
                payload.nonce.len()
            )));
        }

        match payload.cipher {
            CipherAlgorithm::Aes256Gcm => {
                let cipher_key = aes_gcm::Key::<Aes256Gcm>::from(dek);
                let cipher = Aes256Gcm::new(&cipher_key);
                let aes_nonce = AesNonce::try_from(payload.nonce.as_slice())
                    .map_err(|e| SecretError::DecryptionError(e.to_string()))?;
                cipher
                    .decrypt(&aes_nonce, payload.ciphertext.as_ref())
                    .map_err(|e| {
                        SecretError::DecryptionError(format!("AES-GCM decryption failed: {e}"))
                    })
            }
            CipherAlgorithm::ChaCha20Poly1305 => {
                let cipher_key = chacha20poly1305::Key::from(dek);
                let cipher = ChaCha20Poly1305::new(&cipher_key);
                let chacha_nonce = ChaChaNonce::try_from(payload.nonce.as_slice())
                    .map_err(|e| SecretError::DecryptionError(e.to_string()))?;
                cipher
                    .decrypt(&chacha_nonce, payload.ciphertext.as_ref())
                    .map_err(|e| {
                        SecretError::DecryptionError(format!(
                            "ChaCha20Poly1305 decryption failed: {e}"
                        ))
                    })
            }
        }
    }
}
