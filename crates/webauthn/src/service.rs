use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use webauthn_rs::Webauthn;
use webauthn_rs::prelude::WebauthnBuilder;
pub use webauthn_rs::prelude::{
    AuthenticationResult, CreationChallengeResponse, DiscoverableAuthentication, DiscoverableKey,
    Passkey, PasskeyAuthentication, PasskeyRegistration, PublicKeyCredential,
    RegisterPublicKeyCredential, RequestChallengeResponse,
};

use crate::config::WebauthnConfig;
use crate::error::WebauthnError;
use crate::policy::WebauthnPolicy;

/// State persisted across WebAuthn authentication ceremony steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "state")]
pub enum WebauthnAuthState {
    /// Known-user authentication with a pre-specified list of allowed passkeys.
    KnownUser(PasskeyAuthentication),
    /// Discoverable (resident key / usernameless / conditional UI) authentication.
    Discoverable(DiscoverableAuthentication),
}

/// Service for generating WebAuthn challenges and verifying registration/authentication responses.
#[derive(Clone)]
pub struct WebauthnService {
    webauthn: Arc<Webauthn>,
    config: WebauthnConfig,
    policy: Arc<WebauthnPolicy>,
}

impl std::fmt::Debug for WebauthnService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebauthnService")
            .field("config", &self.config)
            .field("policy", &self.policy)
            .finish()
    }
}

impl WebauthnService {
    /// Creates a new WebAuthn service with the given configuration and default policy.
    pub fn new(config: WebauthnConfig) -> Result<Self, WebauthnError> {
        Self::with_policy(config, WebauthnPolicy::default())
    }

    /// Creates a new WebAuthn service with the given configuration and custom policy.
    pub fn with_policy(
        config: WebauthnConfig,
        policy: WebauthnPolicy,
    ) -> Result<Self, WebauthnError> {
        let webauthn = WebauthnBuilder::new(&config.rp_id, &config.rp_origin)
            .map_err(WebauthnError::Ceremony)?
            .rp_name(&config.rp_name)
            .build()
            .map_err(WebauthnError::Ceremony)?;

        Ok(Self {
            webauthn: Arc::new(webauthn),
            config,
            policy: Arc::new(policy),
        })
    }

    /// Returns the active WebAuthn configuration.
    pub fn config(&self) -> &WebauthnConfig {
        &self.config
    }

    /// Returns the active WebAuthn policy.
    pub fn policy(&self) -> &WebauthnPolicy {
        &self.policy
    }

    /// Starts passkey registration for a user, returning challenge options and state to persist.
    pub fn start_registration(
        &self,
        user_id: &str,
        username: &str,
        display_name: &str,
        exclude_passkeys: Option<&[Passkey]>,
    ) -> Result<(CreationChallengeResponse, PasskeyRegistration), WebauthnError> {
        let user_uuid =
            Uuid::parse_str(user_id).map_err(|e| WebauthnError::InvalidUserId(e.to_string()))?;

        let exclude_cred_ids =
            exclude_passkeys.map(|passkeys| passkeys.iter().map(|p| p.cred_id().clone()).collect());

        let (challenge, state) = self
            .webauthn
            .start_passkey_registration(user_uuid, username, display_name, exclude_cred_ids)
            .map_err(WebauthnError::Ceremony)?;

        Ok((challenge, state))
    }

    /// Completes passkey registration by verifying client credential response.
    pub fn finish_registration(
        &self,
        response: &RegisterPublicKeyCredential,
        state: &PasskeyRegistration,
    ) -> Result<Passkey, WebauthnError> {
        let passkey = self
            .webauthn
            .finish_passkey_registration(response, state)
            .map_err(WebauthnError::Ceremony)?;

        Ok(passkey)
    }

    /// Starts discoverable passkey authentication (usernameless / resident key / conditional UI).
    pub fn start_discoverable_authentication(
        &self,
    ) -> Result<(RequestChallengeResponse, WebauthnAuthState), WebauthnError> {
        let (challenge, state) = self
            .webauthn
            .start_discoverable_authentication()
            .map_err(WebauthnError::Ceremony)?;

        Ok((challenge, WebauthnAuthState::Discoverable(state)))
    }

    /// Identifies the user from a discoverable authentication response.
    pub fn identify_discoverable_authentication(
        &self,
        response: &PublicKeyCredential,
    ) -> Result<Uuid, WebauthnError> {
        let (user_id, _) = self
            .webauthn
            .identify_discoverable_authentication(response)
            .map_err(WebauthnError::Ceremony)?;

        Ok(user_id)
    }

    /// Completes discoverable passkey authentication.
    pub fn finish_discoverable_authentication(
        &self,
        response: &PublicKeyCredential,
        state: &DiscoverableAuthentication,
        passkeys: &[Passkey],
    ) -> Result<AuthenticationResult, WebauthnError> {
        let discoverable_keys: Vec<DiscoverableKey> =
            passkeys.iter().map(|p| p.clone().into()).collect();
        let auth_result = self
            .webauthn
            .finish_discoverable_authentication(response, state.clone(), &discoverable_keys)
            .map_err(WebauthnError::Ceremony)?;

        Ok(auth_result)
    }

    /// Starts passkey authentication with a known list of allow passkeys.
    pub fn start_authentication(
        &self,
        allow_passkeys: &[Passkey],
    ) -> Result<(RequestChallengeResponse, WebauthnAuthState), WebauthnError> {
        let (challenge, state) = self
            .webauthn
            .start_passkey_authentication(allow_passkeys)
            .map_err(WebauthnError::Ceremony)?;

        Ok((challenge, WebauthnAuthState::KnownUser(state)))
    }

    /// Completes passkey authentication by verifying client credential response.
    pub fn finish_authentication(
        &self,
        response: &PublicKeyCredential,
        state: &PasskeyAuthentication,
    ) -> Result<AuthenticationResult, WebauthnError> {
        let auth_result = self
            .webauthn
            .finish_passkey_authentication(response, state)
            .map_err(WebauthnError::Ceremony)?;

        Ok(auth_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_webauthn_service_with_defaults() {
        let config = WebauthnConfig::default();
        #[allow(clippy::unwrap_used)]
        let service = WebauthnService::new(config).unwrap();
        assert_eq!(service.config().rp_id, "localhost");
        assert_eq!(service.policy().max_credentials_per_user, 10);
    }
}
