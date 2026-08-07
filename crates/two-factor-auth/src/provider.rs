use crate::errors::TwoFactorError;
use crate::types::{TwoFactorChallenge, TwoFactorMethod, TwoFactorResponse};
use async_trait::async_trait;

/// Generic async provider trait for any 2FA mechanism (TOTP, HOTP, U2F, WebAuthn, SMS OTP, Backup Codes).
#[async_trait]
pub trait TwoFactorProvider: Send + Sync {
    /// Identifies the 2FA mechanism supported by this provider.
    fn method(&self) -> TwoFactorMethod;

    /// Issue a new 2FA challenge for a user or session.
    async fn issue_challenge(
        &self,
        user_identifier: &str,
    ) -> Result<TwoFactorChallenge, TwoFactorError>;

    /// Verify a user response against an active challenge or secret context.
    async fn verify_response(
        &self,
        context_secret: &str,
        response: &TwoFactorResponse,
    ) -> Result<bool, TwoFactorError>;
}
