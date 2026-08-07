use crate::policy::WebAuthnPolicy;
use serde::{Deserialize, Serialize};

/// WebAuthn Relying Party configuration options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthnConfig {
    /// Relying Party ID (e.g., `example.com` or `localhost`).
    pub rp_id: String,
    /// Relying Party Origin URL (e.g., `https://example.com` or `http://localhost:8080`).
    pub rp_origin: String,
    /// Human-readable Relying Party Name (e.g., `Backend Kit Auth`).
    pub rp_name: String,
    /// WebAuthn security policy requirements.
    pub policy: WebAuthnPolicy,
}

impl WebAuthnConfig {
    pub fn new(
        rp_id: impl Into<String>,
        rp_origin: impl Into<String>,
        rp_name: impl Into<String>,
    ) -> Self {
        Self {
            rp_id: rp_id.into(),
            rp_origin: rp_origin.into(),
            rp_name: rp_name.into(),
            policy: WebAuthnPolicy::default(),
        }
    }

    pub fn with_policy(mut self, policy: WebAuthnPolicy) -> Self {
        self.policy = policy;
        self
    }
}
