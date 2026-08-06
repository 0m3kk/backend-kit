use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::errors::PasswordError;

/// Supported password hashing algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Algorithm {
    /// Argon2id (OWASP recommended primary password hashing algorithm)
    Argon2id,
    /// Bcrypt (Widely used legacy and modern password hashing)
    Bcrypt,
    /// No-op / Plaintext hasher (FOR TESTING ONLY)
    Noop,
}

impl fmt::Display for Algorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Argon2id => write!(f, "argon2id"),
            Self::Bcrypt => write!(f, "bcrypt"),
            Self::Noop => write!(f, "noop"),
        }
    }
}

impl FromStr for Algorithm {
    type Err = PasswordError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "argon2id" | "argon2" => Ok(Self::Argon2id),
            "bcrypt" | "2b" | "2a" | "2y" => Ok(Self::Bcrypt),
            "noop" | "plaintext" => Ok(Self::Noop),
            _ => Err(PasswordError::UnsupportedAlgorithm(s.to_string())),
        }
    }
}

/// Strongly-typed wrapper around a encoded password hash string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PasswordHash {
    hash: String,
    algorithm: Algorithm,
}

impl PasswordHash {
    /// Creates a new `PasswordHash` after auto-detecting algorithm from the hash string.
    pub fn parse(hash: impl Into<String>) -> Result<Self, PasswordError> {
        let hash_str = hash.into();
        let algorithm = Self::detect_algorithm(&hash_str)?;

        Ok(Self {
            hash: hash_str,
            algorithm,
        })
    }

    /// Construct a `PasswordHash` directly with a known algorithm.
    pub fn new(hash: String, algorithm: Algorithm) -> Self {
        Self { hash, algorithm }
    }

    /// Auto-detect the hashing algorithm from standard PHC or bcrypt hash prefix.
    pub fn detect_algorithm(hash: &str) -> Result<Algorithm, PasswordError> {
        if hash.starts_with("$argon2id$")
            || hash.starts_with("$argon2i$")
            || hash.starts_with("$argon2d$")
        {
            Ok(Algorithm::Argon2id)
        } else if hash.starts_with("$2a$")
            || hash.starts_with("$2b$")
            || hash.starts_with("$2y$")
            || hash.starts_with("$bcrypt$")
        {
            Ok(Algorithm::Bcrypt)
        } else if hash.starts_with("$noop$") {
            Ok(Algorithm::Noop)
        } else {
            Err(PasswordError::InvalidFormat(
                "Unrecognized or unsupported password hash format scheme".to_string(),
            ))
        }
    }

    /// Get the underlying formatted hash string representation.
    pub fn as_str(&self) -> &str {
        &self.hash
    }

    /// Consume self and return the string representation.
    pub fn into_string(self) -> String {
        self.hash
    }

    /// Get the detected algorithm for this password hash.
    pub fn algorithm(&self) -> Algorithm {
        self.algorithm
    }
}

impl fmt::Display for PasswordHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.hash)
    }
}
