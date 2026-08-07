use crate::types::PolicyViolation;
use thiserror::Error;

/// Core error type for password policy validation and operations.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum PolicyError {
    /// Password failed one or more policy requirements.
    #[error("Password failed validation policy: {0:?}")]
    Violations(Vec<PolicyViolation>),

    /// Password generator failed to produce a valid password.
    #[error("Failed to generate password meeting policy constraints: {0}")]
    GeneratorFailed(String),

    /// Have I Been Pwned API breach lookup failed.
    #[error("HIBP breach check failed: {0}")]
    HibpLookupFailed(String),

    /// Invalid configuration for policy parameters.
    #[error("Invalid password policy configuration: {0}")]
    InvalidConfiguration(String),
}
