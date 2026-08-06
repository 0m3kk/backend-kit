use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::errors::SecretError;

/// Validated path identifier for secrets (e.g., `prod/db/password`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SecretPath(pub String);

impl SecretPath {
    pub fn new(path: impl Into<String>) -> Result<Self, SecretError> {
        let raw = path.into();
        let trimmed = raw.trim_matches('/');

        if trimmed.is_empty() {
            return Err(SecretError::InvalidPath(
                "Secret path cannot be empty".to_string(),
            ));
        }

        for segment in trimmed.split('/') {
            if segment.is_empty() {
                return Err(SecretError::InvalidPath(
                    "Secret path cannot contain empty segments ('//')".to_string(),
                ));
            }
            if !segment
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
            {
                return Err(SecretError::InvalidPath(format!(
                    "Invalid characters in path segment '{segment}'. Allowed: [a-zA-Z0-9_.-]"
                )));
            }
        }

        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn starts_with(&self, prefix: &SecretPath) -> bool {
        self.0.starts_with(prefix.as_str())
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Deref for SecretPath {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for SecretPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<&str> for SecretPath {
    type Error = SecretError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for SecretPath {
    type Error = SecretError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Secure wrapper for sensitive secret values. Redacts contents in `Debug` output
/// and clears memory buffers via zeroize on drop.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretValue(Vec<u8>);

impl SecretValue {
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self(bytes.into())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn as_str(&self) -> Result<&str, SecretError> {
        std::str::from_utf8(&self.0)
            .map_err(|e| SecretError::SerializationError(format!("Invalid UTF-8 string: {e}")))
    }

    pub fn into_inner(mut self) -> Vec<u8> {
        std::mem::take(&mut self.0)
    }

    pub fn from_json<T: Serialize>(data: &T) -> Result<Self, SecretError> {
        let bytes =
            serde_json::to_vec(data).map_err(|e| SecretError::SerializationError(e.to_string()))?;
        Ok(Self(bytes))
    }

    pub fn to_json<T: for<'a> Deserialize<'a>>(&self) -> Result<T, SecretError> {
        serde_json::from_slice(&self.0).map_err(|e| SecretError::SerializationError(e.to_string()))
    }
}

impl Drop for SecretValue {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretValue([REDACTED])")
    }
}

impl From<Vec<u8>> for SecretValue {
    fn from(bytes: Vec<u8>) -> Self {
        Self::new(bytes)
    }
}

impl From<&[u8]> for SecretValue {
    fn from(bytes: &[u8]) -> Self {
        Self::new(bytes.to_vec())
    }
}

impl From<String> for SecretValue {
    fn from(s: String) -> Self {
        Self::new(s.into_bytes())
    }
}

impl From<&str> for SecretValue {
    fn from(s: &str) -> Self {
        Self::new(s.as_bytes().to_vec())
    }
}

impl std::str::FromStr for SecretValue {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.as_bytes().to_vec()))
    }
}

/// Supported symmetric AEAD encryption algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum CipherAlgorithm {
    #[default]
    Aes256Gcm,
    ChaCha20Poly1305,
}

/// Ciphertext payload containing encryption metadata, nonce, and tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedPayload {
    pub cipher: CipherAlgorithm,
    pub key_id: String,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub tag: Option<Vec<u8>>,
}

/// Represents a header/metadata snapshot of a secret without exposing the decrypted payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretHeader {
    pub path: SecretPath,
    pub version: u64,
    pub tags: HashMap<String, String>,
    pub created_at: SystemTime,
    pub expires_at: Option<SystemTime>,
    pub is_deleted: bool,
}

impl SecretHeader {
    pub fn is_expired(&self) -> bool {
        if let Some(exp) = self.expires_at {
            SystemTime::now() >= exp
        } else {
            false
        }
    }
}

/// Represents a decrypted secret entry returned from the store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretEntry {
    pub path: SecretPath,
    pub value: SecretValue,
    pub version: u64,
    pub tags: HashMap<String, String>,
    pub created_at: SystemTime,
    pub expires_at: Option<SystemTime>,
}

impl SecretEntry {
    pub fn is_expired(&self) -> bool {
        if let Some(exp) = self.expires_at {
            SystemTime::now() >= exp
        } else {
            false
        }
    }

    pub fn to_header(&self) -> SecretHeader {
        SecretHeader {
            path: self.path.clone(),
            version: self.version,
            tags: self.tags.clone(),
            created_at: self.created_at,
            expires_at: self.expires_at,
            is_deleted: false,
        }
    }
}

/// Options controlling secret creation/updates.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetSecretOptions {
    pub ttl: Option<Duration>,
    pub tags: HashMap<String, String>,
}

impl SetSecretOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    pub fn with_tags(mut self, tags: HashMap<String, String>) -> Self {
        self.tags = tags;
        self
    }
}

/// Options for searching and listing secrets.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListSecretOptions {
    pub prefix: Option<SecretPath>,
    pub tag_filter: HashMap<String, String>,
    pub include_deleted: bool,
    pub limit: Option<usize>,
}

impl ListSecretOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_prefix(mut self, prefix: SecretPath) -> Self {
        self.prefix = Some(prefix);
        self
    }

    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tag_filter.insert(key.into(), value.into());
        self
    }

    pub fn include_deleted(mut self, include: bool) -> Self {
        self.include_deleted = include;
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}
