use crate::errors::PolicyError;
use crate::policy::PasswordPolicy;
use rand::seq::{IndexedRandom, SliceRandom};

/// Password generator that creates cryptographically secure random passwords meeting policy constraints.
#[derive(Debug, Clone, Default)]
pub struct PasswordGenerator {
    max_attempts: usize,
}

impl PasswordGenerator {
    /// Create a new password generator instance.
    pub fn new() -> Self {
        Self { max_attempts: 100 }
    }

    /// Customize the maximum retry attempts for password generation.
    pub fn with_max_attempts(mut self, attempts: usize) -> Self {
        self.max_attempts = attempts;
        self
    }

    /// Generate a policy-compliant password using default recommended length (at least 16 characters or policy min_length).
    pub fn generate(&self, policy: &PasswordPolicy) -> Result<String, PolicyError> {
        let target_len = std::cmp::max(policy.min_length, 16);
        self.generate_with_length(policy, target_len)
    }

    /// Generate a policy-compliant password of exact specified length.
    pub fn generate_with_length(
        &self,
        policy: &PasswordPolicy,
        length: usize,
    ) -> Result<String, PolicyError> {
        if length < policy.min_length {
            return Err(PolicyError::InvalidConfiguration(format!(
                "Requested length ({}) is less than policy min_length ({})",
                length, policy.min_length
            )));
        }
        if length > policy.max_length {
            return Err(PolicyError::InvalidConfiguration(format!(
                "Requested length ({}) exceeds policy max_length ({})",
                length, policy.max_length
            )));
        }

        let uppercase_pool: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ".chars().collect();
        let lowercase_pool: Vec<char> = "abcdefghijklmnopqrstuvwxyz".chars().collect();
        let digit_pool: Vec<char> = "0123456789".chars().collect();
        let special_pool: Vec<char> = policy.special_characters.chars().collect();

        let required_chars =
            policy.min_uppercase + policy.min_lowercase + policy.min_digits + policy.min_special;
        if length < required_chars {
            return Err(PolicyError::InvalidConfiguration(format!(
                "Requested length ({}) is insufficient to fulfill total required character counts ({})",
                length, required_chars
            )));
        }

        let mut combined_pool = Vec::new();
        if policy.min_uppercase > 0 || required_chars == 0 {
            combined_pool.extend_from_slice(&uppercase_pool);
        }
        if policy.min_lowercase > 0 || required_chars == 0 {
            combined_pool.extend_from_slice(&lowercase_pool);
        }
        if policy.min_digits > 0 || required_chars == 0 {
            combined_pool.extend_from_slice(&digit_pool);
        }
        if (policy.min_special > 0 || required_chars == 0) && !special_pool.is_empty() {
            combined_pool.extend_from_slice(&special_pool);
        }

        if combined_pool.is_empty() {
            return Err(PolicyError::InvalidConfiguration(
                "Character pool is empty based on current policy settings".to_string(),
            ));
        }

        let mut rng = rand::rng();

        for attempt in 1..=self.max_attempts {
            let mut chars = Vec::with_capacity(length);

            // Mandatory uppercase
            for _ in 0..policy.min_uppercase {
                if let Some(&c) = uppercase_pool.choose(&mut rng) {
                    chars.push(c);
                }
            }

            // Mandatory lowercase
            for _ in 0..policy.min_lowercase {
                if let Some(&c) = lowercase_pool.choose(&mut rng) {
                    chars.push(c);
                }
            }

            // Mandatory digits
            for _ in 0..policy.min_digits {
                if let Some(&c) = digit_pool.choose(&mut rng) {
                    chars.push(c);
                }
            }

            // Mandatory special symbols
            for _ in 0..policy.min_special {
                if let Some(&c) = special_pool.choose(&mut rng) {
                    chars.push(c);
                }
            }

            // Fill remaining characters
            while chars.len() < length {
                if let Some(&c) = combined_pool.choose(&mut rng) {
                    chars.push(c);
                } else {
                    break;
                }
            }

            // Shuffle characters securely
            chars.shuffle(&mut rng);
            let candidate: String = chars.into_iter().collect();

            // Validate generated password against full policy rules
            if policy.validate(&candidate).is_ok() {
                return Ok(candidate);
            }

            if attempt == self.max_attempts {
                return Err(PolicyError::GeneratorFailed(format!(
                    "Exceeded maximum attempts ({}) to generate compliant password",
                    self.max_attempts
                )));
            }
        }

        Err(PolicyError::GeneratorFailed(
            "Unexpected end of password generation loop".to_string(),
        ))
    }
}
