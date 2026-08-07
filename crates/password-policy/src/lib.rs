pub mod common_passwords;
pub mod errors;
pub mod policy;
pub mod rule;
pub mod types;

#[cfg(feature = "generator")]
pub mod generator;

#[cfg(feature = "hibp")]
pub mod hibp;

pub use common_passwords::{COMMON_PASSWORDS, is_common_password};
pub use errors::PolicyError;
pub use policy::{DEFAULT_SPECIAL_CHARS, PasswordPolicy, PasswordPolicyBuilder};
pub use rule::Rule;
pub use types::{PasswordStrength, PolicyViolation, UserContext, ValidationReport};

#[cfg(feature = "generator")]
pub use generator::PasswordGenerator;

#[cfg(feature = "hibp")]
pub use hibp::{AsyncBreachChecker, HibpClient};
