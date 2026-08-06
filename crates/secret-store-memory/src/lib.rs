use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use tokio::sync::RwLock;

use secret_store::{
    CipherAlgorithm, EncryptedPayload, KeyProvider, ListSecretOptions, SecretCrypto, SecretEntry,
    SecretError, SecretHeader, SecretPath, SecretStore, SecretValue, SetSecretOptions,
    StaticKeyProvider,
};

#[derive(Debug, Clone)]
struct StoredVersion {
    version: u64,
    payload: EncryptedPayload,
    created_at: SystemTime,
    expires_at: Option<SystemTime>,
}

#[derive(Debug, Clone)]
struct StoredSecretRecord {
    path: SecretPath,
    active_version: u64,
    max_version: u64,
    tags: HashMap<String, String>,
    is_deleted: bool,
    versions: HashMap<u64, StoredVersion>,
}

/// In-memory concurrent implementation of `SecretStore`.
#[derive(Clone)]
pub struct MemorySecretStore {
    key_provider: Arc<dyn KeyProvider>,
    active_key_id: String,
    default_cipher: CipherAlgorithm,
    records: Arc<RwLock<HashMap<String, StoredSecretRecord>>>,
}

impl MemorySecretStore {
    /// Create a new `MemorySecretStore` with a custom `KeyProvider` and active key ID.
    pub fn new(
        key_provider: Arc<dyn KeyProvider>,
        active_key_id: impl Into<String>,
        default_cipher: CipherAlgorithm,
    ) -> Self {
        Self {
            key_provider,
            active_key_id: active_key_id.into(),
            default_cipher,
            records: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Helper to construct a simple in-memory secret store using a single 32-byte master key.
    pub fn with_master_key(master_key: Vec<u8>) -> Result<Self, SecretError> {
        let key_id = "master-key-0";
        let provider = StaticKeyProvider::new().with_key(key_id, master_key)?;
        Ok(Self::new(
            Arc::new(provider),
            key_id,
            CipherAlgorithm::Aes256Gcm,
        ))
    }
}

#[async_trait]
impl SecretStore for MemorySecretStore {
    async fn get(&self, path: &SecretPath) -> Result<Option<SecretEntry>, SecretError> {
        let guard = self.records.read().await;

        let record = match guard.get(path.as_str()) {
            Some(r) if !r.is_deleted => r,
            _ => return Ok(None),
        };

        let ver = match record.versions.get(&record.active_version) {
            Some(v) => v,
            None => return Ok(None),
        };

        if ver.expires_at.is_some_and(|exp| SystemTime::now() >= exp) {
            return Ok(None);
        }

        let master_key = self.key_provider.get_key(&ver.payload.key_id)?;
        let decrypted_bytes = SecretCrypto::decrypt(&ver.payload, &master_key)?;

        Ok(Some(SecretEntry {
            path: path.clone(),
            value: SecretValue::new(decrypted_bytes),
            version: ver.version,
            tags: record.tags.clone(),
            created_at: ver.created_at,
            expires_at: ver.expires_at,
        }))
    }

    async fn get_version(
        &self,
        path: &SecretPath,
        version: u64,
    ) -> Result<Option<SecretEntry>, SecretError> {
        let guard = self.records.read().await;

        let record = match guard.get(path.as_str()) {
            Some(r) => r,
            None => return Ok(None),
        };

        let ver = match record.versions.get(&version) {
            Some(v) => v,
            None => return Ok(None),
        };

        if ver.expires_at.is_some_and(|exp| SystemTime::now() >= exp) {
            return Ok(None);
        }

        let master_key = self.key_provider.get_key(&ver.payload.key_id)?;
        let decrypted_bytes = SecretCrypto::decrypt(&ver.payload, &master_key)?;

        Ok(Some(SecretEntry {
            path: path.clone(),
            value: SecretValue::new(decrypted_bytes),
            version: ver.version,
            tags: record.tags.clone(),
            created_at: ver.created_at,
            expires_at: ver.expires_at,
        }))
    }

    async fn set(
        &self,
        path: SecretPath,
        value: SecretValue,
        options: SetSecretOptions,
    ) -> Result<SecretEntry, SecretError> {
        let master_key = self.key_provider.get_key(&self.active_key_id)?;
        let payload = SecretCrypto::encrypt(
            self.default_cipher,
            &self.active_key_id,
            &master_key,
            value.as_bytes(),
        )?;

        let now = SystemTime::now();
        let expires_at = options.ttl.map(|d| now + d);

        let mut guard = self.records.write().await;

        let record = guard
            .entry(path.as_str().to_string())
            .or_insert_with(|| StoredSecretRecord {
                path: path.clone(),
                active_version: 0,
                max_version: 0,
                tags: HashMap::new(),
                is_deleted: false,
                versions: HashMap::new(),
            });

        record.is_deleted = false;
        record.max_version += 1;
        let new_ver_num = record.max_version;
        record.active_version = new_ver_num;

        if !options.tags.is_empty() {
            record.tags = options.tags.clone();
        }

        let stored_version = StoredVersion {
            version: new_ver_num,
            payload,
            created_at: now,
            expires_at,
        };

        record.versions.insert(new_ver_num, stored_version);

        Ok(SecretEntry {
            path,
            value,
            version: new_ver_num,
            tags: record.tags.clone(),
            created_at: now,
            expires_at,
        })
    }

    async fn delete(&self, path: &SecretPath) -> Result<bool, SecretError> {
        let mut guard = self.records.write().await;
        if let Some(record) = guard.get_mut(path.as_str()).filter(|r| !r.is_deleted) {
            record.is_deleted = true;
            return Ok(true);
        }
        Ok(false)
    }

    async fn list(&self, options: ListSecretOptions) -> Result<Vec<SecretHeader>, SecretError> {
        let guard = self.records.read().await;
        let now = SystemTime::now();
        let mut headers = Vec::new();

        for record in guard.values() {
            if record.is_deleted && !options.include_deleted {
                continue;
            }

            if options
                .prefix
                .as_ref()
                .is_some_and(|prefix| !record.path.starts_with(prefix))
            {
                continue;
            }

            let matches_tags = options
                .tag_filter
                .iter()
                .all(|(k, v)| record.tags.get(k) == Some(v));

            if !matches_tags {
                continue;
            }

            let ver = match record.versions.get(&record.active_version) {
                Some(v) => v,
                None => continue,
            };

            if ver.expires_at.is_some_and(|exp| now >= exp) && !options.include_deleted {
                continue;
            }

            headers.push(SecretHeader {
                path: record.path.clone(),
                version: record.active_version,
                tags: record.tags.clone(),
                created_at: ver.created_at,
                expires_at: ver.expires_at,
                is_deleted: record.is_deleted,
            });

            if options.limit.is_some_and(|limit| headers.len() >= limit) {
                break;
            }
        }

        Ok(headers)
    }

    async fn rotate_key(&self, old_key_id: &str, new_key_id: &str) -> Result<u64, SecretError> {
        let old_master_key = self.key_provider.get_key(old_key_id)?;
        let new_master_key = self.key_provider.get_key(new_key_id)?;

        let mut guard = self.records.write().await;
        let mut count = 0u64;

        for record in guard.values_mut() {
            for ver in record.versions.values_mut() {
                if ver.payload.key_id == old_key_id {
                    let decrypted = SecretCrypto::decrypt(&ver.payload, &old_master_key)?;
                    let re_encrypted = SecretCrypto::encrypt(
                        ver.payload.cipher,
                        new_key_id,
                        &new_master_key,
                        &decrypted,
                    )?;
                    ver.payload = re_encrypted;
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    async fn clean_expired(&self, limit: Option<usize>) -> Result<u64, SecretError> {
        let mut guard = self.records.write().await;
        let now = SystemTime::now();
        let mut purged = 0u64;
        let max_limit = limit.unwrap_or(usize::MAX);

        let mut empty_paths = Vec::new();

        for (path_str, record) in guard.iter_mut() {
            let expired_versions: Vec<u64> = record
                .versions
                .iter()
                .filter_map(|(ver_num, ver)| {
                    if ver.expires_at.is_some_and(|exp| now >= exp) {
                        Some(*ver_num)
                    } else {
                        None
                    }
                })
                .collect();

            for ver_num in expired_versions {
                record.versions.remove(&ver_num);
                purged += 1;
                if purged as usize >= max_limit {
                    break;
                }
            }

            if record.versions.is_empty() {
                empty_paths.push(path_str.clone());
            }

            if purged as usize >= max_limit {
                break;
            }
        }

        for path_str in empty_paths {
            guard.remove(&path_str);
        }

        Ok(purged)
    }
}
