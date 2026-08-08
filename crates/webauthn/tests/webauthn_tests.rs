#![allow(clippy::unwrap_used, clippy::expect_used)]

use secret_store::memory::MemorySecretStore;
use secret_store::{CipherAlgorithm, KEY_LEN, KeyRing, MasterKey};
use std::sync::Arc;
use webauthn::{
    AttestationConveyancePreference, AuthenticatorAttachment, ResidentKeyRequirement,
    UserVerificationPolicy, WebAuthnAuthenticator, WebAuthnConfig, WebAuthnPolicy,
};

fn create_test_setup() -> Arc<MemorySecretStore> {
    let master_key = MasterKey::new(1, [6u8; KEY_LEN]);
    let keyring = KeyRing::new([master_key]).unwrap();
    Arc::new(MemorySecretStore::new(keyring, CipherAlgorithm::Aes256Gcm))
}

#[tokio::test]
async fn test_webauthn_authenticator_initialization() {
    let store = create_test_setup();
    let config = WebAuthnConfig::new("localhost", "http://localhost:8080", "Test App");

    let auth = WebAuthnAuthenticator::new(store, config.clone()).unwrap();
    assert_eq!(auth.config().rp_id, "localhost");
    assert_eq!(auth.config().rp_name, "Test App");
    assert_eq!(
        auth.config().policy.user_verification,
        UserVerificationPolicy::Preferred
    );
}

#[tokio::test]
async fn test_start_passkey_registration_with_default_policy() {
    let store = create_test_setup();
    let config = WebAuthnConfig::new("localhost", "http://localhost:8080", "Test App");
    let auth = WebAuthnAuthenticator::new(store, config).unwrap();

    let (challenge, _state) = auth
        .start_registration("user_webauthn_1", "alice", "Alice Smith")
        .await
        .unwrap();

    assert_eq!(challenge.public_key.rp.id, "localhost");
    assert_eq!(challenge.public_key.user.name, "alice");
    assert_eq!(challenge.public_key.user.display_name, "Alice Smith");
    assert_eq!(challenge.public_key.timeout, Some(60_000));
}

#[tokio::test]
async fn test_webauthn_policy_enforcement() {
    let store = create_test_setup();
    let policy = WebAuthnPolicy::builder()
        .user_verification(UserVerificationPolicy::Required)
        .authenticator_attachment(AuthenticatorAttachment::Platform)
        .resident_key(ResidentKeyRequirement::Required)
        .attestation(AttestationConveyancePreference::Direct)
        .timeout_ms(120_000)
        .build();

    let config = WebAuthnConfig::new("localhost", "http://localhost:8080", "Test App")
        .with_policy(policy.clone());

    let auth = WebAuthnAuthenticator::new(store, config).unwrap();

    let (challenge, _state) = auth
        .start_registration("user_policy_test", "bob", "Bob Jones")
        .await
        .unwrap();

    let selection = challenge.public_key.authenticator_selection.unwrap();

    assert_eq!(
        selection.authenticator_attachment,
        Some(AuthenticatorAttachment::Platform)
    );
    assert_eq!(
        selection.user_verification,
        UserVerificationPolicy::Required
    );
    assert_eq!(
        selection.resident_key,
        Some(ResidentKeyRequirement::Required)
    );
    assert!(selection.require_resident_key);
    assert!(matches!(
        challenge.public_key.attestation,
        Some(AttestationConveyancePreference::Direct)
    ));
    assert_eq!(challenge.public_key.timeout, Some(120_000));
}

#[tokio::test]
async fn test_passkey_list_and_delete() {
    let store = create_test_setup();
    let config = WebAuthnConfig::new("localhost", "http://localhost:8080", "Test App");
    let auth = WebAuthnAuthenticator::new(store, config).unwrap();
    let user_id = "user_webauthn_2";

    let passkeys = auth.list_passkeys(user_id).await.unwrap();
    assert!(passkeys.is_empty());

    let deleted = auth.delete_passkey(user_id, b"non_existent").await.unwrap();
    assert!(!deleted);
}

#[test]
fn test_webauthn_policy_strict_platform() {
    let policy = WebAuthnPolicy::strict_platform();
    assert_eq!(policy.user_verification, UserVerificationPolicy::Required);
    assert_eq!(
        policy.authenticator_attachment,
        Some(AuthenticatorAttachment::Platform)
    );
    assert_eq!(policy.resident_key, ResidentKeyRequirement::Required);
}

#[test]
fn test_webauthn_policy_flexible() {
    let policy = WebAuthnPolicy::flexible();
    assert_eq!(policy.user_verification, UserVerificationPolicy::Preferred);
    assert!(policy.authenticator_attachment.is_none());
    assert_eq!(policy.resident_key, ResidentKeyRequirement::Preferred);
}

#[test]
fn test_webauthn_policy_builder_platform_only() {
    let policy = WebAuthnPolicy::builder().platform_only().build();
    assert_eq!(
        policy.authenticator_attachment,
        Some(AuthenticatorAttachment::Platform)
    );
    assert_eq!(policy.user_verification, UserVerificationPolicy::Required);
}

#[test]
fn test_webauthn_policy_builder_cross_platform_only() {
    let policy = WebAuthnPolicy::builder().cross_platform_only().build();
    assert_eq!(
        policy.authenticator_attachment,
        Some(AuthenticatorAttachment::CrossPlatform)
    );
}

#[test]
fn test_webauthn_policy_builder_require_resident_key() {
    let required = WebAuthnPolicy::builder().require_resident_key(true).build();
    assert_eq!(required.resident_key, ResidentKeyRequirement::Required);

    let discouraged = WebAuthnPolicy::builder()
        .require_resident_key(false)
        .build();
    assert_eq!(
        discouraged.resident_key,
        ResidentKeyRequirement::Discouraged
    );
}

#[test]
fn test_webauthn_policy_builder_timeout() {
    let policy = WebAuthnPolicy::builder().timeout_ms(120_000).build();
    assert_eq!(policy.timeout_ms, 120_000);
}

#[test]
fn test_webauthn_config_new_defaults() {
    let config = WebAuthnConfig::new("example.com", "https://example.com", "My App");
    assert_eq!(config.rp_id, "example.com");
    assert_eq!(config.rp_origin, "https://example.com");
    assert_eq!(config.rp_name, "My App");
    // Default policy
    assert_eq!(
        config.policy.user_verification,
        UserVerificationPolicy::Preferred
    );
}

#[tokio::test]
async fn test_webauthn_authenticator_invalid_origin() {
    let store = create_test_setup();
    let config = WebAuthnConfig::new("localhost", "not-a-valid-url", "Test App");
    let result = WebAuthnAuthenticator::new(store, config);
    assert!(result.is_err());
}
