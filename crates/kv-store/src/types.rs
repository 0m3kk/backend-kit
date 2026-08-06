use std::fmt;
use std::ops::Deref;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

/// Binary key wrapper for key-value operations.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Key(pub Vec<u8>);

impl Key {
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self(bytes.into())
    }

    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.0).ok()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl From<Vec<u8>> for Key {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

impl From<&[u8]> for Key {
    fn from(v: &[u8]) -> Self {
        Self(v.to_vec())
    }
}

impl From<String> for Key {
    fn from(s: String) -> Self {
        Self(s.into_bytes())
    }
}

impl From<&str> for Key {
    fn from(s: &str) -> Self {
        Self(s.as_bytes().to_vec())
    }
}

impl Deref for Key {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<[u8]> for Key {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Ok(s) = std::str::from_utf8(&self.0) {
            write!(f, "{s}")
        } else {
            write!(f, "{:?}", self.0)
        }
    }
}

/// Binary value payload wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Value(pub Vec<u8>);

impl Value {
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self(bytes.into())
    }

    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.0).ok()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }

    pub fn from_json<T: Serialize>(data: &T) -> Result<Self, serde_json::Error> {
        let bytes = serde_json::to_vec(data)?;
        Ok(Self(bytes))
    }

    pub fn to_json<'a, T: Deserialize<'a>>(&'a self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.0)
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

impl From<&[u8]> for Value {
    fn from(v: &[u8]) -> Self {
        Self(v.to_vec())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self(s.into_bytes())
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self(s.as_bytes().to_vec())
    }
}

impl Deref for Value {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<[u8]> for Value {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Represents a single Key-Value entry with optional expiration timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvEntry {
    pub key: Key,
    pub value: Value,
    pub expires_at: Option<SystemTime>,
}

impl KvEntry {
    pub fn new(key: impl Into<Key>, value: impl Into<Value>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            expires_at: None,
        }
    }

    pub fn with_expires_at(mut self, expires_at: Option<SystemTime>) -> Self {
        self.expires_at = expires_at;
        self
    }

    pub fn is_expired(&self) -> bool {
        if let Some(exp) = self.expires_at {
            SystemTime::now() >= exp
        } else {
            false
        }
    }
}

/// Options controlling a `set` mutation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetOptions {
    /// Time to live after insertion.
    pub ttl: Option<Duration>,
    /// Set only if key does NOT exist (NX condition).
    pub if_not_exists: bool,
    /// Set only if key already exists (XX condition).
    pub if_exists: bool,
}

impl SetOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    pub fn if_not_exists(mut self) -> Self {
        self.if_not_exists = true;
        self.if_exists = false;
        self
    }

    pub fn if_exists(mut self) -> Self {
        self.if_exists = true;
        self.if_not_exists = false;
        self
    }
}

/// Operations for atomic batch updates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchOp {
    Put {
        key: Key,
        value: Value,
        options: SetOptions,
    },
    Delete {
        key: Key,
    },
}

/// Options for key range and prefix scanning.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanOptions {
    /// Match keys starting with this prefix.
    pub prefix: Option<Key>,
    /// Inclusive or exclusive start key bound.
    pub start: Option<Key>,
    /// Inclusive or exclusive end key bound.
    pub end: Option<Key>,
    /// Maximum entries to return.
    pub limit: Option<usize>,
    /// Scan in reverse (descending) order.
    pub reverse: bool,
}

impl ScanOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_prefix(mut self, prefix: impl Into<Key>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    pub fn with_range(mut self, start: Option<Key>, end: Option<Key>) -> Self {
        self.start = start;
        self.end = end;
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn reverse(mut self) -> Self {
        self.reverse = true;
        self
    }
}
