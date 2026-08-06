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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_algorithm_display() {
        assert_eq!(Algorithm::Argon2id.to_string(), "argon2id");
        assert_eq!(Algorithm::Bcrypt.to_string(), "bcrypt");
        assert_eq!(Algorithm::Noop.to_string(), "noop");
    }

    #[test]
    fn test_algorithm_parse() {
        assert_eq!(
            "argon2id".parse::<Algorithm>().unwrap(),
            Algorithm::Argon2id
        );
        assert_eq!("argon2".parse::<Algorithm>().unwrap(), Algorithm::Argon2id);
        assert_eq!("bcrypt".parse::<Algorithm>().unwrap(), Algorithm::Bcrypt);
        assert_eq!("2b".parse::<Algorithm>().unwrap(), Algorithm::Bcrypt);
        assert_eq!("2a".parse::<Algorithm>().unwrap(), Algorithm::Bcrypt);
        assert_eq!("2y".parse::<Algorithm>().unwrap(), Algorithm::Bcrypt);
        assert_eq!("noop".parse::<Algorithm>().unwrap(), Algorithm::Noop);
        assert_eq!("plaintext".parse::<Algorithm>().unwrap(), Algorithm::Noop);
        assert!("unknown".parse::<Algorithm>().is_err());
    }

    #[test]
    fn test_password_hash_detect_argon2id() {
        let hash = PasswordHash::parse(
            "$argon2id$v=19$m=65536,t=3,p=4$c29tZXNhbHQ$RGF0YWJhc2VTZWNyZXRLZXlIYXNoVmFsdWU",
        )
        .unwrap();
        assert_eq!(hash.algorithm(), Algorithm::Argon2id);
    }

    #[test]
    fn test_password_hash_detect_bcrypt() {
        let hash =
            PasswordHash::parse("$2b$12$e8Y7Yp3P9D/w/G5eH9T1eeG3Q1.2K8eG3Q1.2K8eG3Q1.2K8eG3Q1")
                .unwrap();
        assert_eq!(hash.algorithm(), Algorithm::Bcrypt);

        let hash_2a =
            PasswordHash::parse("$2a$04$XYZXYZXYZXYZXYZXYZXYOABCDEFGHIJKLMNOPQRSTUVWXYZ012345")
                .unwrap();
        assert_eq!(hash_2a.algorithm(), Algorithm::Bcrypt);
    }

    #[test]
    fn test_password_hash_detect_noop() {
        let hash = PasswordHash::parse("$noop$secret123").unwrap();
        assert_eq!(hash.algorithm(), Algorithm::Noop);
    }

    #[test]
    fn test_password_hash_invalid_format() {
        let err = PasswordHash::parse("invalid_hash_string");
        assert!(matches!(err, Err(PasswordError::InvalidFormat(_))));
    }

    #[test]
    fn test_password_hash_new_and_accessors() {
        let hash = PasswordHash::new("$argon2id$test".to_string(), Algorithm::Argon2id);
        assert_eq!(hash.as_str(), "$argon2id$test");
        assert_eq!(hash.algorithm(), Algorithm::Argon2id);
        assert_eq!(hash.to_string(), "$argon2id$test");
    }

    #[test]
    fn test_password_hash_into_string() {
        let hash = PasswordHash::new("$noop$x".to_string(), Algorithm::Noop);
        assert_eq!(hash.into_string(), "$noop$x");
    }
}
