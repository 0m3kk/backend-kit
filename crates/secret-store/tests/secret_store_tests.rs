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

#[test]
fn test_secret_path_starts_with() -> Result<(), SecretError> {
    let path = SecretPath::new("prod/database/db_password")?;
    let prefix = SecretPath::new("prod/database")?;
    let non_prefix = SecretPath::new("staging")?;

    assert!(path.starts_with(&prefix));
    assert!(!path.starts_with(&non_prefix));

    // Path starts with itself
    let same_prefix = SecretPath::new("prod/database/db_password")?;
    assert!(path.starts_with(&same_prefix));
    Ok(())
}

#[test]
fn test_secret_path_display_and_deref() -> Result<(), SecretError> {
    let path = SecretPath::new("app/config")?;
    assert_eq!(format!("{}", path), "app/config");
    assert_eq!(&*path, "app/config");
    assert_eq!(path.into_inner(), "app/config".to_string());
    Ok(())
}

#[test]
fn test_secret_path_try_from() -> Result<(), SecretError> {
    let path_from_str: SecretPath = "prod/key".try_into()?;
    assert_eq!(path_from_str.as_str(), "prod/key");

    let path_from_string: SecretPath = String::from("prod/key").try_into()?;
    assert_eq!(path_from_string.as_str(), "prod/key");
    Ok(())
}

#[test]
fn test_secret_header_is_expired() {
    use std::collections::HashMap;
    use std::time::{Duration, SystemTime};

    let not_expired = secret_store::SecretHeader {
        path: SecretPath::new("test/path").unwrap(),
        version: 1,
        tags: HashMap::new(),
        created_at: SystemTime::now(),
        expires_at: Some(SystemTime::now() + Duration::from_secs(3600)),
        is_deleted: false,
    };
    assert!(!not_expired.is_expired());

    let already_expired = secret_store::SecretHeader {
        path: SecretPath::new("test/path2").unwrap(),
        version: 1,
        tags: HashMap::new(),
        created_at: SystemTime::now() - Duration::from_secs(7200),
        expires_at: Some(SystemTime::now() - Duration::from_secs(3600)),
        is_deleted: false,
    };
    assert!(already_expired.is_expired());

    let no_expiry = secret_store::SecretHeader {
        path: SecretPath::new("test/path3").unwrap(),
        version: 1,
        tags: HashMap::new(),
        created_at: SystemTime::now(),
        expires_at: None,
        is_deleted: false,
    };
    assert!(!no_expiry.is_expired());
}

#[test]
fn test_secret_entry_is_expired_and_to_header() {
    use std::collections::HashMap;
    use std::time::{Duration, SystemTime};

    let entry = secret_store::SecretEntry {
        path: SecretPath::new("test/entry").unwrap(),
        value: SecretValue::from("secret"),
        version: 3,
        tags: HashMap::from([("env".to_string(), "prod".to_string())]),
        created_at: SystemTime::now(),
        expires_at: Some(SystemTime::now() + Duration::from_secs(3600)),
    };

    assert!(!entry.is_expired());

    let header = entry.to_header();
    assert_eq!(header.path, entry.path);
    assert_eq!(header.version, 3);
    assert_eq!(header.tags.get("env"), Some(&"prod".to_string()));
    assert!(!header.is_deleted);
    assert!(header.expires_at.is_some());
}

#[test]
fn test_secret_value_from_conversions() {
    let from_vec = SecretValue::from(vec![1u8, 2, 3]);
    assert_eq!(from_vec.as_bytes(), &[1, 2, 3]);

    let from_slice = SecretValue::from(&[4u8, 5, 6][..]);
    assert_eq!(from_slice.as_bytes(), &[4, 5, 6]);

    let from_string = SecretValue::from(String::from("hello"));
    assert_eq!(from_string.as_str().unwrap(), "hello");

    let from_str = SecretValue::from("world");
    assert_eq!(from_str.as_str().unwrap(), "world");

    let from_str_parse: SecretValue = "parsed".parse().unwrap();
    assert_eq!(from_str_parse.as_str().unwrap(), "parsed");
}

#[test]
fn test_set_secret_options_builder() {
    use std::collections::HashMap;
    use std::time::Duration;

    let opts = secret_store::SetSecretOptions::new()
        .with_ttl(Duration::from_secs(300))
        .with_tag("env", "staging")
        .with_tag("team", "backend");

    assert_eq!(opts.ttl, Some(Duration::from_secs(300)));
    assert_eq!(opts.tags.get("env"), Some(&"staging".to_string()));
    assert_eq!(opts.tags.get("team"), Some(&"backend".to_string()));

    let custom_tags = HashMap::from([("k".to_string(), "v".to_string())]);
    let opts2 = secret_store::SetSecretOptions::new().with_tags(custom_tags.clone());
    assert_eq!(opts2.tags, custom_tags);
}

#[test]
fn test_list_secret_options_builder() {
    let prefix = SecretPath::new("prod/db").unwrap();
    let opts = secret_store::ListSecretOptions::new()
        .with_prefix(prefix)
        .with_tag("env", "prod")
        .include_deleted(true)
        .with_limit(50);

    assert!(opts.prefix.is_some());
    assert_eq!(opts.tag_filter.get("env"), Some(&"prod".to_string()));
    assert!(opts.include_deleted);
    assert_eq!(opts.limit, Some(50));
}
