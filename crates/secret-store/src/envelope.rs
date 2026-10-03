use rand::Rng;
use rand::rng;

use crate::cipher::{Aes256GcmCipher, Cipher, CryptoError};
use crate::key::Key;
use crate::value::{EncryptedData, SecretBytes};

/// Encrypted envelope containing the data encrypted with a unique DEK,
/// and the DEK itself encrypted with a KEK (Key Encryption Key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvelopeData {
    /// The secret payload encrypted with the unique DEK.
    pub payload_ciphertext: Vec<u8>,
    /// 96-bit nonce used for the DEK payload encryption.
    pub payload_nonce: [u8; 12],
    /// The 256-bit DEK encrypted with the KEK.
    pub encrypted_dek: Vec<u8>,
    /// 96-bit nonce used for the KEK encryption of the DEK.
    pub dek_nonce: [u8; 12],
    /// Optional salt associated with the KEK encryption.
    pub dek_salt: Option<[u8; 16]>,
}

/// Encrypt a plaintext secret using Envelope Encryption.
pub fn encrypt_envelope(
    kek: &dyn Cipher,
    plain: &SecretBytes,
) -> Result<EnvelopeData, CryptoError> {
    let mut dek_bytes = [0u8; 32];
    rng().fill_bytes(&mut dek_bytes);
    let dek_cipher = Aes256GcmCipher::new(Key::from_bytes(dek_bytes));
    let dek_secret = SecretBytes::from_bytes(dek_bytes.to_vec());

    let payload_enc = dek_cipher.encrypt(plain)?;
    let dek_enc = kek.encrypt(&dek_secret)?;

    Ok(EnvelopeData {
        payload_ciphertext: payload_enc.ciphertext,
        payload_nonce: payload_enc.nonce,
        encrypted_dek: dek_enc.ciphertext,
        dek_nonce: dek_enc.nonce,
        dek_salt: dek_enc.salt,
    })
}

/// Decrypt an envelope.
pub fn decrypt_envelope(
    kek: &dyn Cipher,
    envelope: &EnvelopeData,
) -> Result<SecretBytes, CryptoError> {
    let dek_enc = EncryptedData {
        ciphertext: envelope.encrypted_dek.clone(),
        nonce: envelope.dek_nonce,
        salt: envelope.dek_salt,
    };

    let dek_bytes = kek.decrypt(&dek_enc)?;
    let dek_array: [u8; 32] = dek_bytes
        .as_bytes()
        .try_into()
        .map_err(|_| CryptoError::Decrypt)?;
    let dek_cipher = Aes256GcmCipher::new(Key::from_bytes(dek_array));

    let payload_enc = EncryptedData {
        ciphertext: envelope.payload_ciphertext.clone(),
        nonce: envelope.payload_nonce,
        salt: None,
    };

    dek_cipher.decrypt(&payload_enc)
}

/// Re-encrypt only the DEK from an old KEK to a new KEK.
pub fn reencrypt_dek(
    old_kek: &dyn Cipher,
    new_kek: &dyn Cipher,
    encrypted_dek: &[u8],
    dek_nonce: [u8; 12],
    dek_salt: Option<[u8; 16]>,
) -> Result<EncryptedData, CryptoError> {
    let old_dek_enc = EncryptedData {
        ciphertext: encrypted_dek.to_vec(),
        nonce: dek_nonce,
        salt: dek_salt,
    };

    let dek_bytes = old_kek.decrypt(&old_dek_enc)?;
    new_kek.encrypt(&dek_bytes)
}
