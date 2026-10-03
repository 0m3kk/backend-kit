use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use super::EmailSender;
use crate::error::MailerError;
use crate::message::EmailMessage;

/// Mock email sender for development, tests, and CI.
/// Records sent messages in an in-memory queue.
#[derive(Debug, Clone, Default)]
pub struct MockEmailSender {
    sent_messages: Arc<Mutex<Vec<EmailMessage>>>,
}

impl MockEmailSender {
    /// Creates a new mock email sender.
    pub fn new() -> Self {
        Self {
            sent_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Returns a snapshot of all sent messages recorded by this mock.
    pub fn sent_messages(&self) -> Vec<EmailMessage> {
        self.sent_messages
            .lock()
            .map(|msgs| msgs.clone())
            .unwrap_or_default()
    }

    /// Clears recorded messages.
    pub fn clear(&self) {
        if let Ok(mut msgs) = self.sent_messages.lock() {
            msgs.clear();
        }
    }
}

#[async_trait]
impl EmailSender for MockEmailSender {
    async fn send(
        &self,
        from_name: &str,
        from_email: &str,
        message: &EmailMessage,
    ) -> Result<(), MailerError> {
        tracing::info!(
            from = %format!("{from_name} <{from_email}>"),
            to = %message.to,
            subject = %message.subject,
            "[MOCK EMAIL SENDER] Dispatching email"
        );

        if let Ok(mut msgs) = self.sent_messages.lock() {
            msgs.push(message.clone());
        }

        Ok(())
    }
}
