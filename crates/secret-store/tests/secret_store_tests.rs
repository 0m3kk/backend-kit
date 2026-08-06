use secret_store::{
    CipherAlgorithm, KEY_LEN, KeyRing, MasterKey, SecretCrypto, SecretError, SecretPath,
    SecretValue, generate_dek,
};

#[test]
fn test_secret_path_validation() -> Result<(), SecretError> {
    let valid = SecretPath::new("prod/database/db_password")?;
    assert_eq!(valid.as_str(), "prod/database/db_password");

    let valid_slash = SecretPath::new("/app/config/api_key/")?;
    assert_eq!(valid_slash.as_str(), "app/config/api_key");

    assert!(SecretPath::new("").is_err());
    assert!(SecretPath::new("prod//db").is_err());
    assert!(SecretPath::new("prod/db@key").is_err());
    Ok(())
}

#[test]
fn test_secret_value_redaction_and_json() -> Result<(), SecretError> {
    let value = SecretValue::from("super-secret-password");
    let debug_output = format!("{value:?}");
    assert!(debug_output.contains("[REDACTED]"));
    assert!(!debug_output.contains("super-secret-password"));
    assert_eq!(value.as_str()?, "super-secret-password");

    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct Config {
        token: String,
    }

    let cfg = Config {
        token: "xyz123".to_string(),
    };
    let json_val = SecretValue::from_json(&cfg)?;
    let parsed: Config = json_val.to_json()?;
    assert_eq!(cfg, parsed);
    Ok(())
}

#[test]
fn test_master_key_debug_redacted() {
    let mk = MasterKey::new(1, [0xab; KEY_LEN]);
    let debug_str = format!("{mk:?}");
    assert!(debug_str.contains("redacted"));
    assert!(!debug_str.contains("ab"));
}

#[test]
fn test_key_ring_validation_and_unwrapping() -> Result<(), SecretError> {
    assert!(KeyRing::new([]).is_err());

    let mk1 = MasterKey::new(1, [1u8; KEY_LEN]);
    let mk1_dup = MasterKey::new(1, [2u8; KEY_LEN]);
    assert!(KeyRing::new([mk1.clone(), mk1_dup]).is_err());

    let mk2 = MasterKey::new(2, [2u8; KEY_LEN]);
    let keyring = KeyRing::new([mk1, mk2])?;
    assert_eq!(keyring.current_version(), 2);

    let dek = generate_dek()?;
    let (wrapped, ver) = keyring.wrap_dek(&dek)?;
    assert_eq!(ver, 2);

    let unwrapped = keyring.unwrap_dek(2, &wrapped)?;
    assert_eq!(dek, unwrapped);
    Ok(())
}

#[test]
fn test_envelope_crypto_aes_gcm() -> Result<(), SecretError> {
    let keyring = KeyRing::new([MasterKey::new(1, [42u8; KEY_LEN])])?;
    let plaintext = b"envelope secret message";

    let encrypted =
        SecretCrypto::encrypt_envelope(CipherAlgorithm::Aes256Gcm, &keyring, plaintext)?;
    assert_eq!(encrypted.cipher, CipherAlgorithm::Aes256Gcm);
    assert_eq!(encrypted.kek_version, 1);

    let decrypted = SecretCrypto::decrypt_envelope(&encrypted, &keyring)?;
    assert_eq!(decrypted, plaintext);
    Ok(())
}

#[test]
fn test_envelope_crypto_chacha20_poly1305() -> Result<(), SecretError> {
    let keyring = KeyRing::new([MasterKey::new(5, [77u8; KEY_LEN])])?;
    let plaintext = b"chacha envelope secret payload";

    let encrypted =
        SecretCrypto::encrypt_envelope(CipherAlgorithm::ChaCha20Poly1305, &keyring, plaintext)?;
    assert_eq!(encrypted.cipher, CipherAlgorithm::ChaCha20Poly1305);
    assert_eq!(encrypted.kek_version, 5);

    let decrypted = SecretCrypto::decrypt_envelope(&encrypted, &keyring)?;
    assert_eq!(decrypted, plaintext);
    Ok(())
}
