use secret_store::{SecretPath, SecretStore, SecretStoreTx, SecretValue, SetSecretOptions};
use std::sync::Arc;
use url::Url;
use webauthn_rs::prelude::*;
use webauthn_rs_proto::*;

use crate::config::WebAuthnConfig;
use crate::error::WebAuthnError;
use crate::policy::WebAuthnPolicy;

/// Passwordless Passkeys / WebAuthn primary authentication engine backed by `SecretStore` and `webauthn-rs`.
#[derive(Clone)]
pub struct WebAuthnAuthenticator<S: SecretStore> {
    store: Arc<S>,
    webauthn: Arc<Webauthn>,
    config: WebAuthnConfig,
}

impl<S: SecretStore> WebAuthnAuthenticator<S> {
    /// Create a new `WebAuthnAuthenticator` instance with Relying Party configuration and `SecretStore`.
    pub fn new(store: Arc<S>, config: WebAuthnConfig) -> Result<Self, WebAuthnError> {
        let origin_url = Url::parse(&config.rp_origin)
            .map_err(|e| WebAuthnError::ConfigError(format!("Invalid rp_origin URL: {e}")))?;

        let webauthn = WebauthnBuilder::new(&config.rp_id, &origin_url)
            .map_err(|e| WebAuthnError::ConfigError(e.to_string()))?
            .rp_name(&config.rp_name)
            .build()
            .map_err(|e| WebAuthnError::ConfigError(e.to_string()))?;

        Ok(Self {
            store,
            webauthn: Arc::new(webauthn),
            config,
        })
    }

    /// Reference to underlying `SecretStore`.
    pub fn store(&self) -> &Arc<S> {
        &self.store
    }

    /// Reference to internal `Webauthn` engine instance.
    pub fn webauthn(&self) -> &Arc<Webauthn> {
        &self.webauthn
    }

    /// Relying Party configuration.
    pub fn config(&self) -> &WebAuthnConfig {
        &self.config
    }

    // --- Passkey Registration / Enrollment ---

    /// Start WebAuthn Passkey registration using the default configured policy.
    pub async fn start_registration(
        &self,
        user_id: &str,
        username: &str,
        display_name: &str,
    ) -> Result<(CreationChallengeResponse, PasskeyRegistration), WebAuthnError> {
        self.start_registration_with_policy(user_id, username, display_name, &self.config.policy)
            .await
    }

    /// Start WebAuthn Passkey registration with a custom `WebAuthnPolicy`.
    pub async fn start_registration_with_policy(
        &self,
        user_id: &str,
        username: &str,
        display_name: &str,
        policy: &WebAuthnPolicy,
    ) -> Result<(CreationChallengeResponse, PasskeyRegistration), WebAuthnError> {
        let user_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, user_id.as_bytes());

        let existing_passkeys = self.list_passkeys(user_id).await?;
        let exclude_credentials: Vec<CredentialID> = existing_passkeys
            .into_iter()
            .map(|pk| pk.cred_id().clone())
            .collect();

        let (mut challenge, state) = self
            .webauthn
            .start_passkey_registration(
                user_uuid,
                username,
                display_name,
                Some(exclude_credentials),
            )
            .map_err(|e| WebAuthnError::ProtocolError(e.to_string()))?;

        // Apply policy customizations to challenge options
        let mut selection = challenge
            .public_key
            .authenticator_selection
            .unwrap_or_default();
        selection.authenticator_attachment = policy.authenticator_attachment;
        selection.user_verification = policy.user_verification;
        selection.resident_key = Some(policy.resident_key);
        selection.require_resident_key = policy.resident_key == ResidentKeyRequirement::Required;

        challenge.public_key.authenticator_selection = Some(selection);
        challenge.public_key.attestation = Some(policy.attestation.clone());
        challenge.public_key.timeout = Some(policy.timeout_ms);

        Ok((challenge, state))
    }

    /// Complete Passkey registration using credential response from browser `navigator.credentials.create()`.
    /// Validates response and saves new Passkey into `SecretStore` under `webauthn/passkey/{user_id}/{credential_id_hex}`.
    pub async fn finish_registration(
        &self,
        user_id: &str,
        res: &RegisterPublicKeyCredential,
        state: &PasskeyRegistration,
    ) -> Result<Passkey, WebAuthnError> {
        let passkey = self
            .webauthn
            .finish_passkey_registration(res, state)
            .map_err(|e| WebAuthnError::ProtocolError(e.to_string()))?;

        let cred_id_hex = hex_encode(passkey.cred_id().as_slice());
        let path = SecretPath::new(format!("webauthn/passkey/{user_id}/{cred_id_hex}"))
            .map_err(|e| WebAuthnError::StoreError(e.to_string()))?;

        let json = serde_json::to_string(&passkey)
            .map_err(|e| WebAuthnError::SerializationError(e.to_string()))?;

        self.store
            .set(path, SecretValue::from(json), SetSecretOptions::default())
            .await
            .map_err(|e| WebAuthnError::StoreError(e.to_string()))?;

        Ok(passkey)
    }

    // --- Passkey Authentication / Passwordless Login ---

    /// Start WebAuthn Passkey authentication using default configured policy.
    pub async fn start_authentication(
        &self,
        user_id: &str,
    ) -> Result<(RequestChallengeResponse, PasskeyAuthentication), WebAuthnError> {
        self.start_authentication_with_policy(user_id, &self.config.policy)
            .await
    }

    /// Start WebAuthn Passkey authentication with a custom `WebAuthnPolicy`.
    pub async fn start_authentication_with_policy(
        &self,
        user_id: &str,
        policy: &WebAuthnPolicy,
    ) -> Result<(RequestChallengeResponse, PasskeyAuthentication), WebAuthnError> {
        let passkeys = self.list_passkeys(user_id).await?;
        if passkeys.is_empty() {
            return Err(WebAuthnError::PasskeyNotFound(format!(
                "No passkeys registered for user {user_id}"
            )));
        }

        let (mut challenge, state) = self
            .webauthn
            .start_passkey_authentication(&passkeys)
            .map_err(|e| WebAuthnError::ProtocolError(e.to_string()))?;

        challenge.public_key.user_verification = policy.user_verification;
        challenge.public_key.timeout = Some(policy.timeout_ms);

        Ok((challenge, state))
    }

    /// Complete WebAuthn Passkey authentication using assertion response from browser `navigator.credentials.get()`.
    /// Validates passkey signature and checks counter against replay attacks.
    pub async fn finish_authentication(
        &self,
        user_id: &str,
        res: &PublicKeyCredential,
        state: &PasskeyAuthentication,
    ) -> Result<AuthenticationResult, WebAuthnError> {
        let auth_result = self
            .webauthn
            .finish_passkey_authentication(res, state)
            .map_err(|e| WebAuthnError::ProtocolError(e.to_string()))?;

        // Update stored Passkey credential counter in SecretStore
        let cred_id_hex = hex_encode(res.id.as_bytes());
        let path = SecretPath::new(format!("webauthn/passkey/{user_id}/{cred_id_hex}"))
            .map_err(|e| WebAuthnError::StoreError(e.to_string()))?;

        let passkeys = self.list_passkeys(user_id).await?;
        if let Some(mut updated_passkey) = passkeys
            .into_iter()
            .find(|pk| pk.cred_id() == res.id.as_bytes())
        {
            updated_passkey.update_credential(&auth_result);
            let json = serde_json::to_string(&updated_passkey)
                .map_err(|e| WebAuthnError::SerializationError(e.to_string()))?;

            self.store
                .set(path, SecretValue::from(json), SetSecretOptions::default())
                .await
                .map_err(|e| WebAuthnError::StoreError(e.to_string()))?;
        }

        Ok(auth_result)
    }

    // --- Passkey Management ---

    /// List all active registered Passkeys for a user.
    pub async fn list_passkeys(&self, user_id: &str) -> Result<Vec<Passkey>, WebAuthnError> {
        let prefix = SecretPath::new(format!("webauthn/passkey/{user_id}/"))
            .map_err(|e| WebAuthnError::StoreError(e.to_string()))?;

        let options = secret_store::ListSecretOptions::default().with_prefix(prefix);
        let headers = self
            .store
            .list(options)
            .await
            .map_err(|e| WebAuthnError::StoreError(e.to_string()))?;

        let mut passkeys = Vec::new();
        for header in headers {
            if let Ok(Some(entry)) = self.store.get(&header.path).await {
                let passkey_res = entry
                    .value
                    .as_str()
                    .map_err(|e| WebAuthnError::StoreError(e.to_string()))
                    .and_then(|val_str| {
                        serde_json::from_str::<Passkey>(val_str)
                            .map_err(|e| WebAuthnError::SerializationError(e.to_string()))
                    });

                if let Ok(passkey) = passkey_res {
                    passkeys.push(passkey);
                }
            }
        }

        Ok(passkeys)
    }

    /// Delete a registered Passkey for a user by credential ID bytes.
    pub async fn delete_passkey(
        &self,
        user_id: &str,
        cred_id: &[u8],
    ) -> Result<bool, WebAuthnError> {
        let cred_id_hex = hex_encode(cred_id);
        let path = SecretPath::new(format!("webauthn/passkey/{user_id}/{cred_id_hex}"))
            .map_err(|e| WebAuthnError::StoreError(e.to_string()))?;

        self.store
            .delete(&path)
            .await
            .map_err(|e| WebAuthnError::StoreError(e.to_string()))
    }

    // -----------------------------------------------------------------------
    // Transactional methods (uses SecretStoreTx<Conn> trait)
    // -----------------------------------------------------------------------

    /// Complete Passkey registration within an external transaction.
    pub async fn finish_registration_tx<Conn: Send>(
        &self,
        conn: &mut Conn,
        user_id: &str,
        res: &RegisterPublicKeyCredential,
        state: &PasskeyRegistration,
    ) -> Result<Passkey, WebAuthnError>
    where
        S: SecretStoreTx<Conn>,
    {
        let passkey = self
            .webauthn
            .finish_passkey_registration(res, state)
            .map_err(|e| WebAuthnError::ProtocolError(e.to_string()))?;

        let cred_id_hex = hex_encode(passkey.cred_id().as_slice());
        let path = SecretPath::new(format!("webauthn/passkey/{user_id}/{cred_id_hex}"))
            .map_err(|e| WebAuthnError::StoreError(e.to_string()))?;

        let json = serde_json::to_string(&passkey)
            .map_err(|e| WebAuthnError::SerializationError(e.to_string()))?;

        <S as SecretStoreTx<Conn>>::set_tx(
            &*self.store,
            conn,
            path,
            SecretValue::from(json),
            SetSecretOptions::default(),
        )
        .await
        .map_err(|e| WebAuthnError::StoreError(e.to_string()))?;

        Ok(passkey)
    }

    /// Start WebAuthn Passkey authentication within an external transaction.
    pub async fn start_authentication_tx<Conn: Send>(
        &self,
        conn: &mut Conn,
        user_id: &str,
    ) -> Result<(RequestChallengeResponse, PasskeyAuthentication), WebAuthnError>
    where
        S: SecretStoreTx<Conn>,
    {
        self.start_authentication_with_policy_tx(conn, user_id, &self.config.policy)
            .await
    }

    /// Start WebAuthn Passkey authentication with a custom policy within an external transaction.
    pub async fn start_authentication_with_policy_tx<Conn: Send>(
        &self,
        conn: &mut Conn,
        user_id: &str,
        policy: &WebAuthnPolicy,
    ) -> Result<(RequestChallengeResponse, PasskeyAuthentication), WebAuthnError>
    where
        S: SecretStoreTx<Conn>,
    {
        let passkeys = self.list_passkeys_tx(conn, user_id).await?;
        if passkeys.is_empty() {
            return Err(WebAuthnError::PasskeyNotFound(format!(
                "No passkeys registered for user {user_id}"
            )));
        }

        let (mut challenge, state) = self
            .webauthn
            .start_passkey_authentication(&passkeys)
            .map_err(|e| WebAuthnError::ProtocolError(e.to_string()))?;

        challenge.public_key.user_verification = policy.user_verification;
        challenge.public_key.timeout = Some(policy.timeout_ms);

        Ok((challenge, state))
    }

    /// Complete WebAuthn Passkey authentication within an external transaction.
    pub async fn finish_authentication_tx<Conn: Send>(
        &self,
        conn: &mut Conn,
        user_id: &str,
        res: &PublicKeyCredential,
        state: &PasskeyAuthentication,
    ) -> Result<AuthenticationResult, WebAuthnError>
    where
        S: SecretStoreTx<Conn>,
    {
        let auth_result = self
            .webauthn
            .finish_passkey_authentication(res, state)
            .map_err(|e| WebAuthnError::ProtocolError(e.to_string()))?;

        let cred_id_hex = hex_encode(res.id.as_bytes());
        let path = SecretPath::new(format!("webauthn/passkey/{user_id}/{cred_id_hex}"))
            .map_err(|e| WebAuthnError::StoreError(e.to_string()))?;

        let passkeys = self.list_passkeys_tx(conn, user_id).await?;
        if let Some(mut updated_passkey) = passkeys
            .into_iter()
            .find(|pk| pk.cred_id() == res.id.as_bytes())
        {
            updated_passkey.update_credential(&auth_result);
            let json = serde_json::to_string(&updated_passkey)
                .map_err(|e| WebAuthnError::SerializationError(e.to_string()))?;

            <S as SecretStoreTx<Conn>>::set_tx(
                &*self.store,
                conn,
                path,
                SecretValue::from(json),
                SetSecretOptions::default(),
            )
            .await
            .map_err(|e| WebAuthnError::StoreError(e.to_string()))?;
        }

        Ok(auth_result)
    }

    /// List all active registered Passkeys for a user within an external transaction.
    pub async fn list_passkeys_tx<Conn: Send>(
        &self,
        conn: &mut Conn,
        user_id: &str,
    ) -> Result<Vec<Passkey>, WebAuthnError>
    where
        S: SecretStoreTx<Conn>,
    {
        let prefix = SecretPath::new(format!("webauthn/passkey/{user_id}/"))
            .map_err(|e| WebAuthnError::StoreError(e.to_string()))?;

        let options = secret_store::ListSecretOptions::default().with_prefix(prefix);
        let headers = <S as SecretStoreTx<Conn>>::list_tx(&*self.store, conn, options)
            .await
            .map_err(|e| WebAuthnError::StoreError(e.to_string()))?;

        let mut passkeys = Vec::new();
        for header in headers {
            if let Ok(Some(entry)) =
                <S as SecretStoreTx<Conn>>::get_tx(&*self.store, conn, &header.path).await
            {
                let passkey_res = entry
                    .value
                    .as_str()
                    .map_err(|e| WebAuthnError::StoreError(e.to_string()))
                    .and_then(|val_str| {
                        serde_json::from_str::<Passkey>(val_str)
                            .map_err(|e| WebAuthnError::SerializationError(e.to_string()))
                    });

                if let Ok(passkey) = passkey_res {
                    passkeys.push(passkey);
                }
            }
        }

        Ok(passkeys)
    }

    /// Delete a registered Passkey within an external transaction.
    pub async fn delete_passkey_tx<Conn: Send>(
        &self,
        conn: &mut Conn,
        user_id: &str,
        cred_id: &[u8],
    ) -> Result<bool, WebAuthnError>
    where
        S: SecretStoreTx<Conn>,
    {
        let cred_id_hex = hex_encode(cred_id);
        let path = SecretPath::new(format!("webauthn/passkey/{user_id}/{cred_id_hex}"))
            .map_err(|e| WebAuthnError::StoreError(e.to_string()))?;

        <S as SecretStoreTx<Conn>>::delete_tx(&*self.store, conn, &path)
            .await
            .map_err(|e| WebAuthnError::StoreError(e.to_string()))
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_encode_basic() {
        assert_eq!(hex_encode(&[0xab, 0xcd, 0xef]), "abcdef");
    }

    #[test]
    fn test_hex_encode_zeros() {
        assert_eq!(hex_encode(&[0x00, 0x00]), "0000");
    }

    #[test]
    fn test_hex_encode_ff() {
        assert_eq!(hex_encode(&[0xff, 0xff]), "ffff");
    }

    #[test]
    fn test_hex_encode_empty() {
        assert_eq!(hex_encode(&[]), "");
    }

    #[test]
    fn test_hex_encode_single_byte() {
        assert_eq!(hex_encode(&[0x0a]), "0a");
        assert_eq!(hex_encode(&[0x10]), "10");
    }
}
