//! Integration test for the key module.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use secret_store::{CryptoError, Key, SecretBytes, derive_key_from_passphrase};

fn zero_key() -> [u8; 32] {
    [0u8; 32]
}

#[test]
fn from_bytes_roundtrips() {
    let key = Key::<32>::from_bytes(zero_key());
    assert_eq!(key.as_bytes(), &[0u8; 32]);
}

#[test]
fn hex_key_decodes() {
    let hex = "00".repeat(32);
    let key = Key::<32>::from_str_encoded(&hex).unwrap();
    assert_eq!(key.as_bytes(), &[0u8; 32]);
}

#[test]
fn base64_key_decodes() {
    let b64 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    let key = Key::<32>::from_str_encoded(b64).unwrap();
    assert_eq!(key.as_bytes(), &[0u8; 32]);
}

#[test]
fn raw_printable_key_is_rejected() {
    let result = Key::<32>::from_str_encoded("hunter2hunter2hunter2hunter2");
    assert!(matches!(result, Err(CryptoError::InvalidEncodedKey)));
}

#[test]
fn passphrase_derivation_is_deterministic_for_same_salt() {
    let pass = SecretBytes::from_utf8("hunter2");
    let salt = [7u8; 16];
    let a = derive_key_from_passphrase(&pass, &salt).unwrap();
    let b = derive_key_from_passphrase(&pass, &salt).unwrap();
    assert_eq!(a.as_bytes(), b.as_bytes());
}
