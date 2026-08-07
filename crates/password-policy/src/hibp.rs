use crate::errors::PolicyError;
use async_trait::async_trait;
use sha1::{Digest, Sha1};

/// Asynchronous breach checker interface for inspecting compromised password databases.
#[async_trait]
pub trait AsyncBreachChecker: Send + Sync {
    /// Check if a password appears in known data breaches. Returns `Ok(Some(count))` with breach occurrences or `Ok(None)` if clean.
    async fn check_breach(&self, password: &str) -> Result<Option<u64>, PolicyError>;
}

/// Have I Been Pwned (HIBP) k-Anonymity API client.
#[derive(Debug, Clone)]
pub struct HibpClient {
    client: reqwest::Client,
    api_url: String,
}

impl Default for HibpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HibpClient {
    /// Create a new HIBP client with default parameters.
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("backend-kit-password-policy/1.0")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            client,
            api_url: "https://api.pwnedpasswords.com/range".to_string(),
        }
    }

    /// Construct HIBP client with custom base API URL.
    pub fn with_api_url(mut self, url: impl Into<String>) -> Self {
        self.api_url = url.into();
        self
    }
}

#[async_trait]
impl AsyncBreachChecker for HibpClient {
    async fn check_breach(&self, password: &str) -> Result<Option<u64>, PolicyError> {
        let mut hasher = Sha1::new();
        hasher.update(password.as_bytes());
        let hash_bytes = hasher.finalize();
        let hex_hash: String = hash_bytes.iter().map(|b| format!("{:02X}", b)).collect();

        if hex_hash.len() < 5 {
            return Err(PolicyError::HibpLookupFailed(
                "SHA1 hash length invalid".to_string(),
            ));
        }

        let prefix = &hex_hash[..5];
        let suffix = &hex_hash[5..];

        let url = format!("{}/{}", self.api_url, prefix);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| PolicyError::HibpLookupFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(PolicyError::HibpLookupFailed(format!(
                "HIBP API returned status code: {}",
                response.status()
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| PolicyError::HibpLookupFailed(e.to_string()))?;

        for line in body.lines() {
            let parts: Vec<&str> = line.trim().split(':').collect();
            if parts.len() == 2 && parts[0].eq_ignore_ascii_case(suffix) {
                let count = parts[1].parse::<u64>().map_err(|e| {
                    PolicyError::HibpLookupFailed(format!("Failed to parse breach count: {}", e))
                })?;
                return Ok(Some(count));
            }
        }

        Ok(None)
    }
}
