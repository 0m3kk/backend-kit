pub mod mock;
pub mod smtp;

use async_trait::async_trait;
pub use mock::MockEmailSender;
pub use smtp::SmtpEmailSender;

use crate::error::MailerError;
use crate::message::EmailMessage;

/// Trait abstraction for email delivery transports.
#[async_trait]
pub trait EmailSender: Send + Sync {
    /// Sends an email message.
    async fn send(
        &self,
        from_name: &str,
        from_email: &str,
        message: &EmailMessage,
    ) -> Result<(), MailerError>;
}
