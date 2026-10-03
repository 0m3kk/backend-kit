//! Integration test for the cipher module.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use secret_store::{Aes256GcmCipher, Cipher, CryptoError, Key, SecretBytes};

fn cipher_with(byte: u8) -> Aes256GcmCipher {
    Aes256GcmCipher::new(Key::from_bytes([byte; 32]))
}

#[test]
fn encrypt_decrypt_roundtrips() {
    let cipher = cipher_with(1);
    let plain = SecretBytes::from_utf8("hunter2");

    let enc = cipher.encrypt(&plain).unwrap();
    let dec = cipher.decrypt(&enc).unwrap();

    assert_eq!(dec.as_bytes(), b"hunter2");
}

#[test]
fn encrypt_produces_fresh_nonce_and_ciphertext() {
    let cipher = cipher_with(1);
    let plain = SecretBytes::from_utf8("hunter2");

    let a = cipher.encrypt(&plain).unwrap();
    let b = cipher.encrypt(&plain).unwrap();

    assert_ne!(a.nonce, b.nonce);
    assert_ne!(a.ciphertext, b.ciphertext);
    assert!(a.salt.is_none());
}

#[test]
fn tampered_ciphertext_fails_decryption() {
    let cipher = cipher_with(1);
    let plain = SecretBytes::from_utf8("hunter2");

    let mut enc = cipher.encrypt(&plain).unwrap();
    enc.ciphertext[0] ^= 0xFF;

    assert!(matches!(cipher.decrypt(&enc), Err(CryptoError::Decrypt)));
}

#[test]
fn wrong_key_fails_decryption() {
    let enc = cipher_with(1)
        .encrypt(&SecretBytes::from_utf8("hunter2"))
        .unwrap();

    let wrong = cipher_with(2);
    assert!(matches!(wrong.decrypt(&enc), Err(CryptoError::Decrypt)));
}
