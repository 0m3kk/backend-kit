use aes_gcm::Aes256Gcm;
use aes_gcm::aead::{Aead, KeyInit};
use rand::Rng;
use rand::rng;
use zeroize::Zeroizing;

use crate::key::Key;
use crate::value::{EncryptedData, SecretBytes};

type Nonce = aes_gcm::aead::Nonce<Aes256Gcm>;

/// Errors from key handling or encryption/decryption.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    /// A key was provided with an invalid length.
    InvalidKeyLength {
        /// The expected key length in bytes.
        expected: usize,
        /// The actual length in bytes.
        got: usize,
    },
    /// A key string was not valid hex or base64 of the right length.
    InvalidEncodedKey,
    /// The argon2id key derivation failed.
    Argon2(argon2::Error),
    /// The AEAD cipher failed to encrypt.
    Encrypt(aead::Error),
    /// Decryption failed authentication: wrong key, tampered or corrupted
    /// ciphertext, or a reused nonce.
    Decrypt,
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidKeyLength { expected, got } => write!(
                f,
                "invalid key length: expected {expected} bytes, got {got}"
            ),
            Self::InvalidEncodedKey => {
                f.write_str("key is not valid hex or base64 of the right length")
            }
            Self::Argon2(e) => write!(f, "argon2id key derivation failed: {e}"),
            Self::Encrypt(e) => write!(f, "encryption failed: {e}"),
            Self::Decrypt => f.write_str("decryption failed authentication"),
        }
    }
}

impl std::error::Error for CryptoError {}

/// Interface for encrypting and decrypting byte payloads.
///
/// A cipher is synchronous and must be usable from multiple threads; a single
/// instance can be shared across async calls.
pub trait Cipher: Send + Sync {
    /// Encrypt a plaintext value, returning the opaque ciphertext and the
    /// nonce (and salt, for passphrase-derived keys) needed to decrypt it.
    fn encrypt(&self, plain: &SecretBytes) -> Result<EncryptedData, CryptoError>;

    /// Decrypt an [`EncryptedData`] back to a plaintext value.
    ///
    /// Returns [`CryptoError::Decrypt`] when authentication fails — wrong
    /// key, tampered or corrupted ciphertext, or a reused nonce.
    fn decrypt(&self, enc: &EncryptedData) -> Result<SecretBytes, CryptoError>;
}

/// AES-256-GCM cipher.
///
/// Each encryption draws a fresh random 96-bit nonce and produces
/// authenticated ciphertext. The key is zeroized on drop and never leaves this
/// struct.
pub struct Aes256GcmCipher {
    key: Zeroizing<[u8; 32]>,
}

impl Aes256GcmCipher {
    /// Create a cipher from a 256-bit key.
    pub fn new(key: Key<32>) -> Self {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(key.as_bytes());
        Self {
            key: Zeroizing::new(bytes),
        }
    }
}

impl Cipher for Aes256GcmCipher {
    fn encrypt(&self, plain: &SecretBytes) -> Result<EncryptedData, CryptoError> {
        let cipher = Aes256Gcm::new_from_slice(&self.key[..]).map_err(|_| {
            CryptoError::InvalidKeyLength {
                expected: 32,
                got: self.key.len(),
            }
        })?;

        let mut nonce_bytes = [0u8; 12];
        rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from(nonce_bytes);

        let ciphertext = cipher
            .encrypt(&nonce, plain.as_bytes())
            .map_err(CryptoError::Encrypt)?;

        Ok(EncryptedData {
            ciphertext,
            nonce: nonce_bytes,
            salt: None,
        })
    }

    fn decrypt(&self, enc: &EncryptedData) -> Result<SecretBytes, CryptoError> {
        let cipher = Aes256Gcm::new_from_slice(&self.key[..]).map_err(|_| {
            CryptoError::InvalidKeyLength {
                expected: 32,
                got: self.key.len(),
            }
        })?;
        let nonce = Nonce::from(enc.nonce);

        let plain = cipher
            .decrypt(&nonce, enc.ciphertext.as_ref())
            .map_err(|_| CryptoError::Decrypt)?;

        Ok(SecretBytes::from_bytes(plain))
    }
}

impl std::fmt::Debug for Aes256GcmCipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Aes256GcmCipher([REDACTED])")
    }
}
