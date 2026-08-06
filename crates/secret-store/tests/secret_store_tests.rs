use secret_store::{
    CipherAlgorithm, KeyProvider, SecretCrypto, SecretError, SecretPath, SecretValue,
    StaticKeyProvider,
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
fn test_crypto_aes_gcm() -> Result<(), SecretError> {
    let key = vec![0u8; 32];
    let provider = StaticKeyProvider::new().with_key("master-1", key.clone())?;
    let key_bytes = provider.get_key("master-1")?;

    let plaintext = b"hello secret world";
    let encrypted = SecretCrypto::encrypt(
        CipherAlgorithm::Aes256Gcm,
        "master-1",
        &key_bytes,
        plaintext,
    )?;
    assert_eq!(encrypted.cipher, CipherAlgorithm::Aes256Gcm);
    assert_eq!(encrypted.key_id, "master-1");

    let decrypted = SecretCrypto::decrypt(&encrypted, &key_bytes)?;
    assert_eq!(decrypted, plaintext);
    Ok(())
}

#[test]
fn test_crypto_chacha20_poly1305() -> Result<(), SecretError> {
    let key = vec![7u8; 32];
    let provider = StaticKeyProvider::new().with_key("master-chacha", key.clone())?;
    let key_bytes = provider.get_key("master-chacha")?;

    let plaintext = b"chacha secret payload";
    let encrypted = SecretCrypto::encrypt(
        CipherAlgorithm::ChaCha20Poly1305,
        "master-chacha",
        &key_bytes,
        plaintext,
    )?;
    assert_eq!(encrypted.cipher, CipherAlgorithm::ChaCha20Poly1305);

    let decrypted = SecretCrypto::decrypt(&encrypted, &key_bytes)?;
    assert_eq!(decrypted, plaintext);
    Ok(())
}
