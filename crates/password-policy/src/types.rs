use serde::{Deserialize, Serialize};

/// Represents specific policy rule violations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PolicyViolation {
    /// Password length is shorter than required minimum.
    TooShort { min: usize, actual: usize },

    /// Password length exceeds required maximum.
    TooLong { max: usize, actual: usize },

    /// Password contains fewer uppercase characters than required.
    InsufficientUppercase { min: usize, actual: usize },

    /// Password contains fewer lowercase characters than required.
    InsufficientLowercase { min: usize, actual: usize },

    /// Password contains fewer numeric digits than required.
    InsufficientDigits { min: usize, actual: usize },

    /// Password contains fewer special symbols than required.
    InsufficientSpecial { min: usize, actual: usize },

    /// Password contains user context details (e.g., username, email, name).
    ContainsUserContext { field: String },

    /// Password appears on the list of common/compromised passwords.
    CommonPassword,

    /// Password contains an explicitly forbidden character or character set.
    ForbiddenCharacter { char: char },

    /// Password entropy is below the minimum required threshold.
    InsufficientEntropy { min: f64, actual: f64 },

    /// Password contains consecutive repetitive characters exceeding allowed limit.
    RepetitivePatternDetected { char: char, count: usize },

    /// Password contains known sequential keyboard or alphanumeric patterns.
    SequentialPatternDetected { pattern: String },

    /// Password contains whitespace characters when disallowed.
    WhitespaceDisallowed,

    /// Password was found in known data breaches.
    BreachedPassword { count: u64 },

    /// Custom user-defined rule violation.
    Custom { rule: String, message: String },
}

/// Categorization of password strength based on entropy calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PasswordStrength {
    /// Weak password (< 36 bits of entropy).
    Weak,
    /// Medium strength password (36 - 59 bits of entropy).
    Medium,
    /// Strong password (60 - 79 bits of entropy).
    Strong,
    /// Very strong password (>= 80 bits of entropy).
    VeryStrong,
}

impl PasswordStrength {
    /// Classify entropy in bits into a strength category.
    pub fn from_entropy(bits: f64) -> Self {
        if bits < 36.0 {
            Self::Weak
        } else if bits < 60.0 {
            Self::Medium
        } else if bits < 80.0 {
            Self::Strong
        } else {
            Self::VeryStrong
        }
    }
}

/// User details provided to ensure passwords do not derive from user identity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserContext {
    pub username: Option<String>,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub custom_tokens: Vec<String>,
}

impl UserContext {
    /// Create a new empty user context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the username field.
    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// Set the email field.
    pub fn with_email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    /// Set first and last names.
    pub fn with_name(
        mut self,
        first_name: impl Into<String>,
        last_name: impl Into<String>,
    ) -> Self {
        self.first_name = Some(first_name.into());
        self.last_name = Some(last_name.into());
        self
    }

    /// Add a custom user token (e.g., company name, phone number, birth year).
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.custom_tokens.push(token.into());
        self
    }

    /// Extract user context tokens of minimum length to check against candidate passwords.
    pub fn extract_tokens(&self, min_len: usize) -> Vec<String> {
        let mut raw_inputs = Vec::new();

        if let Some(ref u) = self.username {
            raw_inputs.push(u.clone());
        }
        if let Some(ref e) = self.email {
            raw_inputs.push(e.clone());
        }
        if let Some(ref f) = self.first_name {
            raw_inputs.push(f.clone());
        }
        if let Some(ref l) = self.last_name {
            raw_inputs.push(l.clone());
        }
        raw_inputs.extend(self.custom_tokens.clone());

        let mut tokens = Vec::new();

        for input in raw_inputs {
            let lower = input.to_lowercase();

            // Include full input if it satisfies min length
            if lower.chars().count() >= min_len {
                tokens.push(lower.clone());
            }

            // Split into sub-tokens by non-alphanumeric delimiters (e.g. johndoe@gmail.com -> johndoe, gmail, com)
            for part in lower.split(|c: char| !c.is_alphanumeric()) {
                let trimmed = part.trim();
                if trimmed.chars().count() >= min_len && !tokens.iter().any(|t| t == trimmed) {
                    tokens.push(trimmed.to_string());
                }
            }
        }

        tokens
    }
}

/// Comprehensive evaluation report returned by `PasswordPolicy::audit`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationReport {
    /// True if the password passed all policy rules without violations.
    pub is_valid: bool,

    /// List of rules failed during audit.
    pub violations: Vec<PolicyViolation>,

    /// Calculated Shannon/character-set entropy estimation in bits.
    pub entropy_bits: f64,

    /// Overall password strength assessment.
    pub strength: PasswordStrength,
}

impl ValidationReport {
    /// Construct a new validation report.
    pub fn new(violations: Vec<PolicyViolation>, entropy_bits: f64) -> Self {
        let is_valid = violations.is_empty();
        let strength = PasswordStrength::from_entropy(entropy_bits);
        Self {
            is_valid,
            violations,
            entropy_bits,
            strength,
        }
    }

    /// Check whether the validation was completely successful.
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }
}
