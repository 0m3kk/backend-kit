use std::time::Duration;

use secret_store::{
    CipherAlgorithm, KEY_LEN, KeyRing, ListSecretOptions, MasterKey, SecretError, SecretPath,
    SecretStore, SecretValue, SetSecretOptions,
};
use secret_store_memory::MemorySecretStore;

#[tokio::test]
async fn test_memory_secret_store_crud() -> Result<(), SecretError> {
    let store = MemorySecretStore::with_master_key([42u8; KEY_LEN])?;
    let path = SecretPath::new("app/database/password")?;
    let val = SecretValue::from("p@ssword123");

    // Get non-existent
    assert!(store.get(&path).await?.is_none());

    // Set
    let created = store
        .set(
            path.clone(),
            val.clone(),
            SetSecretOptions::new().with_tag("env", "prod"),
        )
        .await?;
    assert_eq!(created.version, 1);
    assert_eq!(created.tags.get("env"), Some(&"prod".to_string()));

    // Get latest
    let fetched = store
        .get(&path)
        .await?
        .ok_or_else(|| SecretError::StoreError("NotFound".to_string()))?;
    assert_eq!(fetched.version, 1);
    assert_eq!(fetched.value.as_str()?, "p@ssword123");

    // Update to v2
    let val2 = SecretValue::from("new_p@ssword456");
    let v2_created = store
        .set(path.clone(), val2, SetSecretOptions::new())
        .await?;
    assert_eq!(v2_created.version, 2);

    // Get v1 & v2 explicitly
    let v1_entry = store
        .get_version(&path, 1)
        .await?
        .ok_or_else(|| SecretError::StoreError("NotFound".to_string()))?;
    assert_eq!(v1_entry.value.as_str()?, "p@ssword123");

    let v2_entry = store
        .get_version(&path, 2)
        .await?
        .ok_or_else(|| SecretError::StoreError("NotFound".to_string()))?;
    assert_eq!(v2_entry.value.as_str()?, "new_p@ssword456");

    // Delete
    assert!(store.delete(&path).await?);
    assert!(store.get(&path).await?.is_none());
    Ok(())
}

#[tokio::test]
async fn test_memory_secret_store_list_and_filter() -> Result<(), SecretError> {
    let store = MemorySecretStore::with_master_key([1u8; KEY_LEN])?;

    store
        .set(
            SecretPath::new("prod/db/pass")?,
            SecretValue::from("v1"),
            SetSecretOptions::new().with_tag("tier", "db"),
        )
        .await?;
    store
        .set(
            SecretPath::new("prod/api/token")?,
            SecretValue::from("v2"),
            SetSecretOptions::new().with_tag("tier", "api"),
        )
        .await?;
    store
        .set(
            SecretPath::new("dev/db/pass")?,
            SecretValue::from("v3"),
            SetSecretOptions::new().with_tag("tier", "db"),
        )
        .await?;

    let list_prod = store
        .list(ListSecretOptions::new().with_prefix(SecretPath::new("prod")?))
        .await?;
    assert_eq!(list_prod.len(), 2);

    let list_db = store
        .list(ListSecretOptions::new().with_tag("tier", "db"))
        .await?;
    assert_eq!(list_db.len(), 2);

    Ok(())
}

#[tokio::test]
async fn test_memory_key_rotation() -> Result<(), SecretError> {
    let k1 = MasterKey::new(1, [10u8; KEY_LEN]);
    let keyring = KeyRing::new([k1])?;

    let store = MemorySecretStore::new(keyring, CipherAlgorithm::Aes256Gcm);

    let path = SecretPath::new("security/master_token")?;
    store
        .set(
            path.clone(),
            SecretValue::from("token_val"),
            SetSecretOptions::new(),
        )
        .await?;

    // Add version 2 master key to KeyRing
    let k2 = MasterKey::new(2, [20u8; KEY_LEN]);
    store.add_master_key(k2).await?;

    // Rotate keys: re-wraps DEKs under version 2
    let re_encrypted_count = store.rotate_key().await?;
    assert_eq!(re_encrypted_count, 1);

    // Verify secret can still be retrieved and decrypted seamlessly
    let secret = store
        .get(&path)
        .await?
        .ok_or_else(|| SecretError::StoreError("NotFound".to_string()))?;
    assert_eq!(secret.value.as_str()?, "token_val");

    Ok(())
}

#[tokio::test]
async fn test_memory_expiration() -> Result<(), SecretError> {
    let store = MemorySecretStore::with_master_key([99u8; KEY_LEN])?;
    let active_path = SecretPath::new("temp/active_key")?;

    store
        .set(
            active_path.clone(),
            SecretValue::from("active_val"),
            SetSecretOptions::new().with_ttl(Duration::from_secs(5)),
        )
        .await?;

    assert!(store.get(&active_path).await?.is_some());

    let exp_path = SecretPath::new("temp/expiring_key")?;
    store
        .set(
            exp_path.clone(),
            SecretValue::from("temp_val"),
            SetSecretOptions::new().with_ttl(Duration::from_millis(20)),
        )
        .await?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(store.get(&exp_path).await?.is_none());

    let cleaned = store.clean_expired(None).await?;
    assert_eq!(cleaned, 1);

    Ok(())
}
