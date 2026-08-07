use crate::common_passwords::is_common_password;
use crate::errors::PolicyError;
use crate::rule::{find_repetitive_pattern, find_sequential_pattern};
use crate::types::{PolicyViolation, UserContext, ValidationReport};
use serde::{Deserialize, Serialize};

/// Standard set of ASCII special characters accepted as symbols.
pub const DEFAULT_SPECIAL_CHARS: &str = "!@#$%^&*()_+-=[]{}|;:,.<>?/~`'\"\\";

/// Core password policy configuration and validator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PasswordPolicy {
    /// Minimum required character length.
    pub min_length: usize,

    /// Maximum allowed character length.
    pub max_length: usize,

    /// Minimum uppercase ASCII/Unicode letters required.
    pub min_uppercase: usize,

    /// Minimum lowercase ASCII/Unicode letters required.
    pub min_lowercase: usize,

    /// Minimum numeric digits required.
    pub min_digits: usize,

    /// Minimum special symbol characters required.
    pub min_special: usize,

    /// Custom set of characters treated as special symbols.
    pub special_characters: String,

    /// Whether spaces and whitespace are allowed in passwords.
    pub allow_whitespace: bool,

    /// Optional minimum bit entropy threshold (e.g., 50.0).
    pub min_entropy_bits: Option<f64>,

    /// Maximum allowed consecutive identical characters (e.g., 3 means "aaaa" fails).
    pub max_consecutive_repeat: Option<usize>,

    /// Whether keyboard or alphanumeric sequences (e.g. "12345", "qwerty") are forbidden.
    pub prohibit_sequential: bool,

    /// Whether to check against the built-in list of common weak passwords.
    pub use_builtin_blocklist: bool,

    /// Custom list of forbidden words/passwords.
    pub custom_blocklist: Vec<String>,

    /// Whether to enforce user context checks (username, email, names).
    pub check_user_context: bool,

    /// Minimum character length for user context tokens to match against.
    pub min_context_token_len: usize,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            min_length: 8,
            max_length: 128,
            min_uppercase: 1,
            min_lowercase: 1,
            min_digits: 1,
            min_special: 1,
            special_characters: DEFAULT_SPECIAL_CHARS.to_string(),
            allow_whitespace: false,
            min_entropy_bits: None,
            max_consecutive_repeat: None,
            prohibit_sequential: false,
            use_builtin_blocklist: true,
            custom_blocklist: Vec::new(),
            check_user_context: true,
            min_context_token_len: 3,
        }
    }
}

impl PasswordPolicy {
    /// Create a fluent builder for constructing custom password policies.
    pub fn builder() -> PasswordPolicyBuilder {
        PasswordPolicyBuilder::new()
    }

    /// OWASP & NIST SP 800-63B compliant password policy preset.
    /// Focuses on length (min 8, max 128), allows spaces, checks blocklists & user context.
    pub fn nist() -> Self {
        Self {
            min_length: 8,
            max_length: 128,
            min_uppercase: 0,
            min_lowercase: 0,
            min_digits: 0,
            min_special: 0,
            special_characters: DEFAULT_SPECIAL_CHARS.to_string(),
            allow_whitespace: true,
            min_entropy_bits: None,
            max_consecutive_repeat: None,
            prohibit_sequential: false,
            use_builtin_blocklist: true,
            custom_blocklist: Vec::new(),
            check_user_context: true,
            min_context_token_len: 3,
        }
    }

    /// OWASP Recommended baseline password policy preset.
    /// Minimum length 10, requires character diversity (upper, lower, digit, special), checks blocklist.
    pub fn owasp() -> Self {
        Self {
            min_length: 10,
            max_length: 128,
            min_uppercase: 1,
            min_lowercase: 1,
            min_digits: 1,
            min_special: 1,
            special_characters: DEFAULT_SPECIAL_CHARS.to_string(),
            allow_whitespace: false,
            min_entropy_bits: None,
            max_consecutive_repeat: None,
            prohibit_sequential: false,
            use_builtin_blocklist: true,
            custom_blocklist: Vec::new(),
            check_user_context: true,
            min_context_token_len: 3,
        }
    }

    /// Strict enterprise password policy preset.
    /// Minimum length 14, min 2 of each character class, min entropy 50 bits, no repeat >= 3, prohibit sequential sequences.
    pub fn strict() -> Self {
        Self {
            min_length: 14,
            max_length: 128,
            min_uppercase: 2,
            min_lowercase: 2,
            min_digits: 2,
            min_special: 2,
            special_characters: DEFAULT_SPECIAL_CHARS.to_string(),
            allow_whitespace: false,
            min_entropy_bits: Some(50.0),
            max_consecutive_repeat: Some(3),
            prohibit_sequential: true,
            use_builtin_blocklist: true,
            custom_blocklist: Vec::new(),
            check_user_context: true,
            min_context_token_len: 3,
        }
    }

    /// Validate candidate password without user context.
    /// Returns `Ok(())` if compliant, or `Err(PolicyError::Violations)` with all failed rules.
    pub fn validate(&self, password: &str) -> Result<(), PolicyError> {
        self.validate_with_context(password, &UserContext::default())
    }

    /// Validate candidate password with user context details.
    /// Returns `Ok(())` if compliant, or `Err(PolicyError::Violations)` with all failed rules.
    pub fn validate_with_context(
        &self,
        password: &str,
        context: &UserContext,
    ) -> Result<(), PolicyError> {
        let report = self.audit_with_context(password, context);
        if report.is_valid {
            Ok(())
        } else {
            Err(PolicyError::Violations(report.violations))
        }
    }

    /// Audit password without user context, producing a detailed `ValidationReport`.
    pub fn audit(&self, password: &str) -> ValidationReport {
        self.audit_with_context(password, &UserContext::default())
    }

    /// Audit password against all policy rules and user context details, returning a `ValidationReport`.
    pub fn audit_with_context(&self, password: &str, context: &UserContext) -> ValidationReport {
        let mut violations = Vec::new();
        let char_count = password.chars().count();

        // 1. Length checks
        if char_count < self.min_length {
            violations.push(PolicyViolation::TooShort {
                min: self.min_length,
                actual: char_count,
            });
        }
        if char_count > self.max_length {
            violations.push(PolicyViolation::TooLong {
                max: self.max_length,
                actual: char_count,
            });
        }

        // 2. Whitespace check
        if !self.allow_whitespace && password.chars().any(|c| c.is_whitespace()) {
            violations.push(PolicyViolation::WhitespaceDisallowed);
        }

        // 3. Character class counts
        let mut uppercase_count = 0;
        let mut lowercase_count = 0;
        let mut digit_count = 0;
        let mut special_count = 0;

        for c in password.chars() {
            if c.is_uppercase() {
                uppercase_count += 1;
            } else if c.is_lowercase() {
                lowercase_count += 1;
            } else if c.is_numeric() {
                digit_count += 1;
            } else if self.special_characters.contains(c) {
                special_count += 1;
            }
        }

        if uppercase_count < self.min_uppercase {
            violations.push(PolicyViolation::InsufficientUppercase {
                min: self.min_uppercase,
                actual: uppercase_count,
            });
        }
        if lowercase_count < self.min_lowercase {
            violations.push(PolicyViolation::InsufficientLowercase {
                min: self.min_lowercase,
                actual: lowercase_count,
            });
        }
        if digit_count < self.min_digits {
            violations.push(PolicyViolation::InsufficientDigits {
                min: self.min_digits,
                actual: digit_count,
            });
        }
        if special_count < self.min_special {
            violations.push(PolicyViolation::InsufficientSpecial {
                min: self.min_special,
                actual: special_count,
            });
        }

        // 4. Consecutive repetitive pattern check
        if let Some(max_repeat) = self.max_consecutive_repeat
            && let Some((ch, count)) = find_repetitive_pattern(password, max_repeat)
        {
            violations.push(PolicyViolation::RepetitivePatternDetected { char: ch, count });
        }

        // 5. Sequential pattern check
        if self.prohibit_sequential
            && let Some(seq) = find_sequential_pattern(password, 3)
        {
            violations.push(PolicyViolation::SequentialPatternDetected { pattern: seq });
        }

        // 6. Blocklist checks
        let lower_pwd = password.to_lowercase();
        if self.use_builtin_blocklist && is_common_password(password) {
            violations.push(PolicyViolation::CommonPassword);
        }
        if self
            .custom_blocklist
            .iter()
            .any(|entry| entry.to_lowercase() == lower_pwd)
        {
            violations.push(PolicyViolation::CommonPassword);
        }

        // 7. User Context check
        if self.check_user_context {
            let tokens = context.extract_tokens(self.min_context_token_len);
            for token in tokens {
                if lower_pwd.contains(&token) {
                    violations.push(PolicyViolation::ContainsUserContext { field: token });
                    break;
                }
            }
        }

        // 8. Entropy calculation & requirement check
        let entropy_bits = Self::calculate_entropy(password);
        if let Some(min_entropy) = self.min_entropy_bits
            && entropy_bits < min_entropy
        {
            violations.push(PolicyViolation::InsufficientEntropy {
                min: min_entropy,
                actual: entropy_bits,
            });
        }

        ValidationReport::new(violations, entropy_bits)
    }

    /// Estimate Shannon/character-set entropy of a candidate password in bits.
    pub fn calculate_entropy(password: &str) -> f64 {
        let char_count = password.chars().count();
        if char_count == 0 {
            return 0.0;
        }

        let mut pool_size: f64 = 0.0;
        let mut has_lower = false;
        let mut has_upper = false;
        let mut has_digit = false;
        let mut has_special = false;
        let mut has_other = false;

        for c in password.chars() {
            if c.is_ascii_lowercase() {
                has_lower = true;
            } else if c.is_ascii_uppercase() {
                has_upper = true;
            } else if c.is_ascii_digit() {
                has_digit = true;
            } else if c.is_ascii_punctuation() {
                has_special = true;
            } else {
                has_other = true;
            }
        }

        if has_lower {
            pool_size += 26.0;
        }
        if has_upper {
            pool_size += 26.0;
        }
        if has_digit {
            pool_size += 10.0;
        }
        if has_special {
            pool_size += 32.0;
        }
        if has_other {
            pool_size += 128.0;
        }

        if pool_size <= 0.0 {
            return 0.0;
        }

        (char_count as f64) * pool_size.log2()
    }
}

/// Fluent builder for constructing custom `PasswordPolicy` instances.
#[derive(Debug, Clone, Default)]
pub struct PasswordPolicyBuilder {
    policy: PasswordPolicy,
}

impl PasswordPolicyBuilder {
    /// Initialize builder with default configuration.
    pub fn new() -> Self {
        Self {
            policy: PasswordPolicy::default(),
        }
    }

    /// Set minimum password character length.
    pub fn min_length(mut self, min: usize) -> Self {
        self.policy.min_length = min;
        self
    }

    /// Set maximum password character length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.policy.max_length = max;
        self
    }

    /// Set required minimum count of uppercase characters.
    pub fn min_uppercase(mut self, min: usize) -> Self {
        self.policy.min_uppercase = min;
        self
    }

    /// Set required minimum count of lowercase characters.
    pub fn min_lowercase(mut self, min: usize) -> Self {
        self.policy.min_lowercase = min;
        self
    }

    /// Set required minimum count of numeric digits.
    pub fn min_digits(mut self, min: usize) -> Self {
        self.policy.min_digits = min;
        self
    }

    /// Set required minimum count of special symbols.
    pub fn min_special(mut self, min: usize) -> Self {
        self.policy.min_special = min;
        self
    }

    /// Set custom set of characters recognized as special symbols.
    pub fn special_characters(mut self, symbols: impl Into<String>) -> Self {
        self.policy.special_characters = symbols.into();
        self
    }

    /// Configure whether whitespace characters are allowed.
    pub fn allow_whitespace(mut self, allow: bool) -> Self {
        self.policy.allow_whitespace = allow;
        self
    }

    /// Set minimum bit entropy threshold requirement.
    pub fn min_entropy_bits(mut self, min_bits: f64) -> Self {
        self.policy.min_entropy_bits = Some(min_bits);
        self
    }

    /// Set maximum allowed consecutive identical characters.
    pub fn max_consecutive_repeat(mut self, max_repeat: usize) -> Self {
        self.policy.max_consecutive_repeat = Some(max_repeat);
        self
    }

    /// Configure whether sequential keyboard or alphanumeric patterns are prohibited.
    pub fn prohibit_sequential(mut self, prohibit: bool) -> Self {
        self.policy.prohibit_sequential = prohibit;
        self
    }

    /// Configure whether to use built-in blocklist of common weak passwords.
    pub fn use_builtin_blocklist(mut self, use_builtin: bool) -> Self {
        self.policy.use_builtin_blocklist = use_builtin;
        self
    }

    /// Add custom words/passwords to the policy blocklist.
    pub fn blocklist(mut self, words: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.policy
            .custom_blocklist
            .extend(words.into_iter().map(|w| w.into()));
        self
    }

    /// Configure whether to check against user context tokens (username, email, names).
    pub fn check_user_context(mut self, check: bool) -> Self {
        self.policy.check_user_context = check;
        self
    }

    /// Build and validate the constructed `PasswordPolicy`.
    pub fn build(self) -> Result<PasswordPolicy, PolicyError> {
        if self.policy.min_length > self.policy.max_length {
            return Err(PolicyError::InvalidConfiguration(format!(
                "min_length ({}) cannot be greater than max_length ({})",
                self.policy.min_length, self.policy.max_length
            )));
        }
        Ok(self.policy)
    }
}
