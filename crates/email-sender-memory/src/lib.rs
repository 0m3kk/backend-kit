use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use email_sender::{Email, EmailError, EmailResponse, EmailSender};
use tokio::sync::RwLock;

/// An in-memory [`EmailSender`] implementation designed for unit testing and development.
///
/// Captures sent emails in memory so tests can inspect sent messages, count sent emails,
/// or simulate transmission errors.
#[derive(Debug, Clone)]
pub struct MemoryEmailSender {
    sent_emails: Arc<RwLock<Vec<Email>>>,
    forced_error: Arc<RwLock<Option<EmailError>>>,
    counter: Arc<AtomicU64>,
}

impl MemoryEmailSender {
    /// Creates a new, empty `MemoryEmailSender`.
    pub fn new() -> Self {
        Self {
            sent_emails: Arc::new(RwLock::new(Vec::new())),
            forced_error: Arc::new(RwLock::new(None)),
            counter: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Sets an error that will be returned on subsequent [`send`] calls.
    /// Set to `None` to clear any forced error.
    pub async fn set_forced_error(&self, error: Option<EmailError>) {
        let mut guard = self.forced_error.write().await;
        *guard = error;
    }

    /// Returns a copy of all emails sent through this sender.
    pub async fn sent_emails(&self) -> Vec<Email> {
        let guard = self.sent_emails.read().await;
        guard.clone()
    }

    /// Returns the number of emails sent through this sender.
    pub async fn count(&self) -> usize {
        let guard = self.sent_emails.read().await;
        guard.len()
    }

    /// Returns the last email sent, if any.
    pub async fn last_sent(&self) -> Option<Email> {
        let guard = self.sent_emails.read().await;
        guard.last().cloned()
    }

    /// Clears all captured sent emails.
    pub async fn clear(&self) {
        let mut guard = self.sent_emails.write().await;
        guard.clear();
    }
}

impl Default for MemoryEmailSender {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EmailSender for MemoryEmailSender {
    async fn send(&self, email: &Email) -> Result<EmailResponse, EmailError> {
        // Check for injected test failure
        let forced_err = self.forced_error.read().await;
        if let Some(err) = forced_err.as_ref() {
            return Err(err.clone());
        }

        let mut emails = self.sent_emails.write().await;
        emails.push(email.clone());

        let id = self.counter.fetch_add(1, Ordering::SeqCst);
        let message_id = format!("mem-{id}");

        Ok(EmailResponse::new(
            Some(message_id),
            Some("Sent via MemoryEmailSender".to_string()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use email_sender::EmailAddress;

    use super::*;

    #[tokio::test]
    async fn test_memory_sender_captures_emails() -> Result<(), EmailError> {
        let sender = MemoryEmailSender::new();

        let email = Email::builder()
            .from(EmailAddress::new("from@test.com")?)
            .to(EmailAddress::new("to@test.com")?)
            .subject("Test Subject")
            .text_body("Test Body")
            .build()?;

        let response = sender.send(&email).await?;
        assert!(response.message_id.is_some());

        assert_eq!(sender.count().await, 1);
        let last = sender.last_sent().await;
        assert!(last.is_some());
        if let Some(last_email) = last {
            assert_eq!(last_email.subject, "Test Subject");
        }

        sender.clear().await;
        assert_eq!(sender.count().await, 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_memory_sender_forced_error() -> Result<(), EmailError> {
        let sender = MemoryEmailSender::new();
        sender
            .set_forced_error(Some(EmailError::TransportError("Network down".to_string())))
            .await;

        let email = Email::builder()
            .from(EmailAddress::new("from@test.com")?)
            .to(EmailAddress::new("to@test.com")?)
            .subject("Test Subject")
            .text_body("Test Body")
            .build()?;

        let result = sender.send(&email).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            EmailError::TransportError("Network down".to_string())
        );

        sender.set_forced_error(None).await;
        assert!(sender.send(&email).await.is_ok());
        Ok(())
    }
}
