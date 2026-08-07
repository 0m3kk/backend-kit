use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum SmsError {
    #[error("SMS send failed: {0}")]
    SendFailed(String),

    #[error("Invalid phone number: {0}")]
    InvalidPhoneNumber(String),

    #[error("Provider error: {0}")]
    ProviderError(String),
}

/// Domain model representing an SMS message payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmsMessage {
    pub recipient: String,
    pub body: String,
}

impl SmsMessage {
    pub fn new(recipient: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            recipient: recipient.into(),
            body: body.into(),
        }
    }
}

/// Universal SMS Sender specification trait providing async SMS delivery.
#[async_trait]
pub trait SmsSender: Send + Sync {
    /// Send an SMS message asynchronously.
    async fn send_sms(&self, message: &SmsMessage) -> Result<(), SmsError>;
}

#[cfg(feature = "memory")]
pub mod memory {
    use super::*;
    use tokio::sync::RwLock;

    /// In-memory SMS sender implementation for testing and development.
    #[derive(Debug, Default)]
    pub struct MemorySmsSender {
        sent: RwLock<Vec<SmsMessage>>,
    }

    impl MemorySmsSender {
        pub fn new() -> Self {
            Self {
                sent: RwLock::new(Vec::new()),
            }
        }

        pub async fn sent_messages(&self) -> Vec<SmsMessage> {
            self.sent.read().await.clone()
        }

        pub async fn last_message_for(&self, recipient: &str) -> Option<SmsMessage> {
            self.sent
                .read()
                .await
                .iter()
                .rev()
                .find(|msg| msg.recipient == recipient)
                .cloned()
        }

        pub async fn clear(&self) {
            self.sent.write().await.clear();
        }
    }

    #[async_trait]
    impl SmsSender for MemorySmsSender {
        async fn send_sms(&self, message: &SmsMessage) -> Result<(), SmsError> {
            self.sent.write().await.push(message.clone());
            Ok(())
        }
    }
}

#[cfg(feature = "memory")]
pub use memory::MemorySmsSender;
