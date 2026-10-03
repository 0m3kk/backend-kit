//! Integration test for the keyring module.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use secret_store::{Cipher, CryptoError, EncryptedData, Keyring, SecretBytes};

struct TestCipher(u8);

impl Cipher for TestCipher {
    fn encrypt(&self, plain: &SecretBytes) -> Result<EncryptedData, CryptoError> {
        Ok(EncryptedData {
            ciphertext: plain.as_bytes().to_vec(),
            nonce: [self.0; 12],
            salt: None,
        })
    }

    fn decrypt(&self, enc: &EncryptedData) -> Result<SecretBytes, CryptoError> {
        Ok(SecretBytes::from_bytes(enc.ciphertext.clone()))
    }
}

#[test]
fn keyring_active_and_historical() {
    let keyring = Keyring::new(2, TestCipher(2)).with_historical(1, TestCipher(1));

    assert_eq!(keyring.active_version(), 2);
    assert!(keyring.get(2).is_some());
    assert!(keyring.get(1).is_some());
    assert!(keyring.get(3).is_none());
}
