use crate::errors::TwoFactorError;
use crate::provider::TwoFactorProvider;
use crate::types::{TwoFactorChallenge, TwoFactorMethod, TwoFactorResponse};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// In-memory mock 2FA service for instant unit testing without infrastructure dependencies.
#[derive(Debug, Clone, Default)]
pub struct MemoryTwoFactorAuth {
    preset_responses: Arc<Mutex<HashMap<String, String>>>,
}

impl MemoryTwoFactorAuth {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn preset_response(&self, challenge_id: impl Into<String>, response: impl Into<String>) {
        if let Ok(mut map) = self.preset_responses.lock() {
            map.insert(challenge_id.into(), response.into());
        }
    }
}

#[async_trait]
impl TwoFactorProvider for MemoryTwoFactorAuth {
    fn method(&self) -> TwoFactorMethod {
        TwoFactorMethod::Totp
    }

    async fn issue_challenge(
        &self,
        user_identifier: &str,
    ) -> Result<TwoFactorChallenge, TwoFactorError> {
        let challenge_id = format!("mem_chal_{user_identifier}");
        Ok(TwoFactorChallenge::new(challenge_id, TwoFactorMethod::Totp))
    }

    async fn verify_response(
        &self,
        challenge_id: &str,
        response: &TwoFactorResponse,
    ) -> Result<bool, TwoFactorError> {
        if let Ok(map) = self.preset_responses.lock() {
            if let Some(expected) = map.get(challenge_id) {
                return Ok(expected == &response.response_data);
            }
        }
        // Default mock acceptance for "123456"
        Ok(response.response_data == "123456")
    }
}
