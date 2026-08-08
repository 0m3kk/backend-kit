use crate::errors::TwoFactorError;
use crate::types::{TwoFactorChallenge, TwoFactorMethod, TwoFactorResponse};
use async_trait::async_trait;
use std::sync::Arc;

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

/// Transactional 2FA provider operations.
///
/// `Conn` represents the connection or transaction handle type. The caller owns the
/// transaction lifecycle — the provider only executes storage operations through the provided handle.
///
/// Not all 2FA providers can implement this trait. Providers with external side effects
/// (e.g., SMS, Email) are excluded because sending messages within a database transaction
/// is unsafe — if the transaction rolls back, the message was already delivered.
#[async_trait]
pub trait TwoFactorProviderTx<Conn: Send>: Send + Sync {
    /// Issue a new 2FA challenge within an external transaction.
    async fn issue_challenge_tx(
        &self,
        conn: &mut Conn,
        user_identifier: &str,
    ) -> Result<TwoFactorChallenge, TwoFactorError>;

    /// Verify a user response within an external transaction.
    async fn verify_response_tx(
        &self,
        conn: &mut Conn,
        context_secret: &str,
        response: &TwoFactorResponse,
    ) -> Result<bool, TwoFactorError>;
}

#[async_trait]
impl<T: TwoFactorProvider + ?Sized> TwoFactorProvider for Arc<T> {
    fn method(&self) -> TwoFactorMethod {
        (**self).method()
    }

    async fn issue_challenge(
        &self,
        user_identifier: &str,
    ) -> Result<TwoFactorChallenge, TwoFactorError> {
        (**self).issue_challenge(user_identifier).await
    }

    async fn verify_response(
        &self,
        context_secret: &str,
        response: &TwoFactorResponse,
    ) -> Result<bool, TwoFactorError> {
        (**self).verify_response(context_secret, response).await
    }
}

#[async_trait]
impl<T: TwoFactorProviderTx<Conn> + ?Sized, Conn: Send> TwoFactorProviderTx<Conn> for Arc<T> {
    async fn issue_challenge_tx(
        &self,
        conn: &mut Conn,
        user_identifier: &str,
    ) -> Result<TwoFactorChallenge, TwoFactorError> {
        (**self).issue_challenge_tx(conn, user_identifier).await
    }

    async fn verify_response_tx(
        &self,
        conn: &mut Conn,
        context_secret: &str,
        response: &TwoFactorResponse,
    ) -> Result<bool, TwoFactorError> {
        (**self)
            .verify_response_tx(conn, context_secret, response)
            .await
    }
}
