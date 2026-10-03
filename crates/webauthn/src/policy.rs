use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use webauthn_rs_proto::{
    AttestationConveyancePreference, AuthenticatorAttachment as ProtoAuthenticatorAttachment,
    ResidentKeyRequirement as ProtoResidentKeyRequirement, UserVerificationPolicy,
};

/// User verification requirement for WebAuthn ceremonies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserVerificationRequirement {
    /// Authenticator MUST verify user (PIN, biometric). Fails if unavailable.
    Required,
    /// Authenticator SHOULD verify user if capable; proceeds with presence if unavailable.
    #[default]
    Preferred,
    /// User verification is not requested / discouraged.
    Discouraged,
}

/// Authenticator attachment requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticatorAttachment {
    /// Any authenticator is accepted (both platform and roaming cross-platform).
    #[default]
    Any,
    /// Platform authenticators only (e.g. Apple TouchID/FaceID, Windows Hello, Android Biometrics).
    Platform,
    /// Roaming / cross-platform authenticators only (e.g. YubiKey, USB/NFC hardware security keys).
    CrossPlatform,
}

/// Resident key / Discoverable credential requirement for passkey registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResidentKeyRequirement {
    /// Resident key is discouraged (non-discoverable credential).
    Discouraged,
    /// Resident key is preferred if supported.
    #[default]
    Preferred,
    /// Resident key is required (creates discoverable passkey for username-less login).
    Required,
}

/// Attestation conveyance preference during registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AttestationPreference {
    /// No attestation is requested (maximizes privacy and speed).
    #[default]
    None,
    /// Direct attestation statement from authenticator hardware.
    Direct,
    /// Indirect / anonymized attestation statement.
    Indirect,
}

/// Comprehensive configurable WebAuthn & Passkey security policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebauthnPolicy {
    /// User verification requirement for registration and authentication.
    #[serde(default)]
    pub user_verification: UserVerificationRequirement,

    /// Authenticator attachment constraint (Platform, CrossPlatform, Any).
    #[serde(default)]
    pub attachment: AuthenticatorAttachment,

    /// Resident key / discoverable credential requirement.
    #[serde(default)]
    pub resident_key: ResidentKeyRequirement,

    /// Attestation conveyance preference.
    #[serde(default)]
    pub attestation: AttestationPreference,

    /// Challenge timeout in seconds for client ceremony completion (default = 60s).
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u32,

    /// Maximum number of WebAuthn credentials allowed per user account (default = 10).
    #[serde(default = "default_max_credentials_per_user")]
    pub max_credentials_per_user: usize,

    /// Whether to enforce authenticator signature counter check for clone detection (default = true).
    #[serde(default = "default_enforce_counter_check")]
    pub enforce_counter_check: bool,
}

const fn default_timeout_secs() -> u32 {
    60
}

const fn default_max_credentials_per_user() -> usize {
    10
}

const fn default_enforce_counter_check() -> bool {
    true
}

impl Default for WebauthnPolicy {
    fn default() -> Self {
        Self {
            user_verification: UserVerificationRequirement::default(),
            attachment: AuthenticatorAttachment::default(),
            resident_key: ResidentKeyRequirement::default(),
            attestation: AttestationPreference::default(),
            timeout_secs: default_timeout_secs(),
            max_credentials_per_user: default_max_credentials_per_user(),
            enforce_counter_check: default_enforce_counter_check(),
        }
    }
}

impl WebauthnPolicy {
    /// Converts `UserVerificationRequirement` to `webauthn_rs_proto` type.
    pub fn to_user_verification_policy(&self) -> UserVerificationPolicy {
        match self.user_verification {
            UserVerificationRequirement::Required => UserVerificationPolicy::Required,
            UserVerificationRequirement::Preferred => UserVerificationPolicy::Preferred,
            UserVerificationRequirement::Discouraged => {
                UserVerificationPolicy::Discouraged_DO_NOT_USE
            }
        }
    }

    /// Converts `AuthenticatorAttachment` to `Option<webauthn_rs_proto::AuthenticatorAttachment>`.
    pub fn to_authenticator_attachment(&self) -> Option<ProtoAuthenticatorAttachment> {
        match self.attachment {
            AuthenticatorAttachment::Any => None,
            AuthenticatorAttachment::Platform => Some(ProtoAuthenticatorAttachment::Platform),
            AuthenticatorAttachment::CrossPlatform => {
                Some(ProtoAuthenticatorAttachment::CrossPlatform)
            }
        }
    }

    /// Converts `ResidentKeyRequirement` to `webauthn_rs_proto` type.
    pub fn to_resident_key_requirement(&self) -> ProtoResidentKeyRequirement {
        match self.resident_key {
            ResidentKeyRequirement::Discouraged => ProtoResidentKeyRequirement::Discouraged,
            ResidentKeyRequirement::Preferred => ProtoResidentKeyRequirement::Preferred,
            ResidentKeyRequirement::Required => ProtoResidentKeyRequirement::Required,
        }
    }

    /// Converts `AttestationPreference` to `webauthn_rs_proto` type.
    pub fn to_attestation_conveyance(&self) -> AttestationConveyancePreference {
        match self.attestation {
            AttestationPreference::None => AttestationConveyancePreference::None,
            AttestationPreference::Direct => AttestationConveyancePreference::Direct,
            AttestationPreference::Indirect => AttestationConveyancePreference::Indirect,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_has_expected_values() {
        let policy = WebauthnPolicy::default();
        assert_eq!(
            policy.user_verification,
            UserVerificationRequirement::Preferred
        );
        assert_eq!(policy.attachment, AuthenticatorAttachment::Any);
        assert_eq!(policy.resident_key, ResidentKeyRequirement::Preferred);
        assert_eq!(policy.attestation, AttestationPreference::None);
        assert_eq!(policy.timeout_secs, 60);
        assert_eq!(policy.max_credentials_per_user, 10);
        assert!(policy.enforce_counter_check);
    }

    #[test]
    fn converts_to_proto_types() {
        let policy = WebauthnPolicy {
            user_verification: UserVerificationRequirement::Required,
            attachment: AuthenticatorAttachment::Platform,
            resident_key: ResidentKeyRequirement::Required,
            attestation: AttestationPreference::Direct,
            ..Default::default()
        };

        assert_eq!(
            policy.to_user_verification_policy(),
            UserVerificationPolicy::Required
        );
        assert_eq!(
            policy.to_authenticator_attachment(),
            Some(ProtoAuthenticatorAttachment::Platform)
        );
        assert_eq!(
            policy.to_resident_key_requirement(),
            ProtoResidentKeyRequirement::Required
        );
        assert!(matches!(
            policy.to_attestation_conveyance(),
            AttestationConveyancePreference::Direct
        ));
    }

    #[test]
    fn serializes_and_deserializes_policy() {
        let json = r#"{
            "user_verification": "required",
            "attachment": "platform",
            "resident_key": "required",
            "attestation": "direct",
            "timeout_secs": 120,
            "max_credentials_per_user": 5,
            "enforce_counter_check": false
        }"#;

        #[allow(clippy::unwrap_used)]
        let policy: WebauthnPolicy = serde_json::from_str(json).unwrap();
        assert_eq!(
            policy.user_verification,
            UserVerificationRequirement::Required
        );
        assert_eq!(policy.attachment, AuthenticatorAttachment::Platform);
        assert_eq!(policy.resident_key, ResidentKeyRequirement::Required);
        assert_eq!(policy.attestation, AttestationPreference::Direct);
        assert_eq!(policy.timeout_secs, 120);
        assert_eq!(policy.max_credentials_per_user, 5);
        assert!(!policy.enforce_counter_check);
    }
}
