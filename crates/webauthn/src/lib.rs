pub mod config;
pub mod error;
pub mod policy;
pub mod service;

pub use config::WebauthnConfig;
pub use error::WebauthnError;
pub use policy::{
    AttestationPreference, AuthenticatorAttachment, ResidentKeyRequirement,
    UserVerificationRequirement, WebauthnPolicy,
};
pub use service::{WebauthnAuthState, WebauthnService};
pub use webauthn_rs::prelude::*;

/// Converts a [`CredentialID`] to its base64url-encoded string representation.
pub fn credential_id_to_string(cred_id: &CredentialID) -> String {
    match serde_json::to_string(cred_id) {
        Ok(s) => s.trim_matches('"').to_string(),
        Err(_) => String::new(),
    }
}

/// Parses a base64url-encoded string into a [`CredentialID`].
pub fn parse_credential_id(s: &str) -> Result<CredentialID, serde_json::Error> {
    serde_json::from_str(&format!("\"{s}\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::unwrap_used)]
    fn encodes_and_decodes_credential_id() {
        let cred_id: CredentialID = parse_credential_id("dGVzdC1jcmVkZW50aWFsLWlk").unwrap();
        let encoded = credential_id_to_string(&cred_id);
        assert_eq!(encoded, "dGVzdC1jcmVkZW50aWFsLWlk");
    }
}
