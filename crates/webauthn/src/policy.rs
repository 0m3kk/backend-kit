use serde::{Deserialize, Serialize};
pub use webauthn_rs_proto::{
    AttestationConveyancePreference, AuthenticatorAttachment, ResidentKeyRequirement,
    UserVerificationPolicy,
};

/// Security policy requirements for WebAuthn passkey registration and authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthnPolicy {
    /// Requirement for user verification (biometrics / PIN). Default is `UserVerificationPolicy::Preferred`.
    pub user_verification: UserVerificationPolicy,
    /// Constraint on authenticator attachment (e.g. `Platform` for TouchID/FaceID, `CrossPlatform` for YubiKey).
    pub authenticator_attachment: Option<AuthenticatorAttachment>,
    /// Requirement for resident key / discoverable passkey. Default is `ResidentKeyRequirement::Preferred`.
    pub resident_key: ResidentKeyRequirement,
    /// Preference for attestation conveyance. Default is `AttestationConveyancePreference::None`.
    pub attestation: AttestationConveyancePreference,
    /// Challenge expiration timeout in milliseconds. Default is 60,000 ms (1 minute).
    pub timeout_ms: u32,
}

impl Default for WebAuthnPolicy {
    fn default() -> Self {
        Self {
            user_verification: UserVerificationPolicy::Preferred,
            authenticator_attachment: None,
            resident_key: ResidentKeyRequirement::Preferred,
            attestation: AttestationConveyancePreference::None,
            timeout_ms: 60_000,
        }
    }
}

impl WebAuthnPolicy {
    /// Create a fluent builder for `WebAuthnPolicy`.
    pub fn builder() -> WebAuthnPolicyBuilder {
        WebAuthnPolicyBuilder::default()
    }

    /// Strict security policy requiring platform biometrics (TouchID/FaceID) and discoverable passkey.
    pub fn strict_platform() -> Self {
        Self::builder()
            .user_verification(UserVerificationPolicy::Required)
            .authenticator_attachment(AuthenticatorAttachment::Platform)
            .resident_key(ResidentKeyRequirement::Required)
            .build()
    }

    /// Flexible security policy accepting any authenticator.
    pub fn flexible() -> Self {
        Self::default()
    }
}

/// Fluent builder for `WebAuthnPolicy`.
#[derive(Debug, Clone, Default)]
pub struct WebAuthnPolicyBuilder {
    policy: WebAuthnPolicy,
}

impl WebAuthnPolicyBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn user_verification(mut self, uv: UserVerificationPolicy) -> Self {
        self.policy.user_verification = uv;
        self
    }

    pub fn authenticator_attachment(mut self, attachment: AuthenticatorAttachment) -> Self {
        self.policy.authenticator_attachment = Some(attachment);
        self
    }

    pub fn platform_only(mut self) -> Self {
        self.policy.authenticator_attachment = Some(AuthenticatorAttachment::Platform);
        self.policy.user_verification = UserVerificationPolicy::Required;
        self
    }

    pub fn cross_platform_only(mut self) -> Self {
        self.policy.authenticator_attachment = Some(AuthenticatorAttachment::CrossPlatform);
        self
    }

    pub fn resident_key(mut self, rk: ResidentKeyRequirement) -> Self {
        self.policy.resident_key = rk;
        self
    }

    pub fn require_resident_key(mut self, required: bool) -> Self {
        self.policy.resident_key = if required {
            ResidentKeyRequirement::Required
        } else {
            ResidentKeyRequirement::Discouraged
        };
        self
    }

    pub fn attestation(mut self, attestation: AttestationConveyancePreference) -> Self {
        self.policy.attestation = attestation;
        self
    }

    pub fn timeout_ms(mut self, timeout_ms: u32) -> Self {
        self.policy.timeout_ms = timeout_ms;
        self
    }

    pub fn build(self) -> WebAuthnPolicy {
        self.policy
    }
}
