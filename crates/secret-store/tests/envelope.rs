//! Integration test for the envelope module.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use secret_store::{
    Cipher, CryptoError, EncryptedData, SecretBytes, decrypt_envelope, encrypt_envelope,
    reencrypt_dek,
};

struct MockKek(u8);

impl Cipher for MockKek {
    fn encrypt(&self, plain: &SecretBytes) -> Result<EncryptedData, CryptoError> {
        Ok(EncryptedData {
            ciphertext: [vec![self.0], plain.as_bytes().to_vec()].concat(),
            nonce: [self.0; 12],
            salt: None,
        })
    }

    fn decrypt(&self, enc: &EncryptedData) -> Result<SecretBytes, CryptoError> {
        if enc.ciphertext.first() == Some(&self.0) {
            Ok(SecretBytes::from_bytes(enc.ciphertext[1..].to_vec()))
        } else {
            Err(CryptoError::Decrypt)
        }
    }
}

#[test]
fn envelope_roundtrips() {
    let kek = MockKek(1);
    let plain = SecretBytes::from_utf8("top secret payload");

    let envelope = encrypt_envelope(&kek, &plain).unwrap();
    let decrypted = decrypt_envelope(&kek, &envelope).unwrap();

    assert_eq!(decrypted.as_bytes(), b"top secret payload");
}

#[test]
fn reencrypt_dek_preserves_payload() {
    let kek1 = MockKek(1);
    let kek2 = MockKek(2);
    let plain = SecretBytes::from_utf8("top secret payload");

    let mut envelope = encrypt_envelope(&kek1, &plain).unwrap();

    let new_dek_enc = reencrypt_dek(
        &kek1,
        &kek2,
        &envelope.encrypted_dek,
        envelope.dek_nonce,
        envelope.dek_salt,
    )
    .unwrap();

    envelope.encrypted_dek = new_dek_enc.ciphertext;
    envelope.dek_nonce = new_dek_enc.nonce;
    envelope.dek_salt = new_dek_enc.salt;

    let decrypted = decrypt_envelope(&kek2, &envelope).unwrap();
    assert_eq!(decrypted.as_bytes(), b"top secret payload");
}
