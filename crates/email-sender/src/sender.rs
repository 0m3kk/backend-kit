use async_trait::async_trait;

use crate::errors::EmailError;
use crate::types::{Email, EmailResponse};

/// Universal interface for asynchronous email senders.
#[async_trait]
pub trait EmailSender: Send + Sync {
    /// Sends a single email.
    async fn send(&self, email: &Email) -> Result<EmailResponse, EmailError>;

    /// Sends a batch of emails. Default implementation sends each email sequentially.
    async fn send_batch(
        &self,
        emails: &[Email],
    ) -> Result<Vec<Result<EmailResponse, EmailError>>, EmailError> {
        let mut results = Vec::with_capacity(emails.len());
        for email in emails {
            results.push(self.send(email).await);
        }
        Ok(results)
    }
}
