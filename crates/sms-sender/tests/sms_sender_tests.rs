#![allow(clippy::unwrap_used, clippy::expect_used)]

use sms_sender::{SmsMessage, SmsSender};

#[test]
fn test_sms_message_new() {
    let msg = SmsMessage::new("+15551234567", "Hello World");
    assert_eq!(msg.recipient, "+15551234567");
    assert_eq!(msg.body, "Hello World");
}

#[test]
fn test_sms_message_from_string() {
    let msg = SmsMessage::new(String::from("+15551234567"), String::from("Test body"));
    assert_eq!(msg.recipient, "+15551234567");
    assert_eq!(msg.body, "Test body");
}

#[test]
fn test_sms_message_clone_and_eq() {
    let msg = SmsMessage::new("+15551234567", "Hello");
    let cloned = msg.clone();
    assert_eq!(msg, cloned);
}

#[cfg(feature = "memory")]
mod memory_tests {
    use super::*;
    use sms_sender::MemorySmsSender;

    #[tokio::test]
    async fn test_memory_sms_sender_new() {
        let sender = MemorySmsSender::new();
        let sent = sender.sent_messages().await;
        assert!(sent.is_empty());
    }

    #[tokio::test]
    async fn test_memory_sms_sender_send_and_retrieve() {
        let sender = MemorySmsSender::new();
        let msg = SmsMessage::new("+15551234567", "Test code: 123456");

        sender.send_sms(&msg).await.unwrap();

        let sent = sender.sent_messages().await;
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].recipient, "+15551234567");
        assert_eq!(sent[0].body, "Test code: 123456");
    }

    #[tokio::test]
    async fn test_memory_sms_sender_last_message_for() {
        let sender = MemorySmsSender::new();
        let msg1 = SmsMessage::new("+15551111111", "First message");
        let msg2 = SmsMessage::new("+15552222222", "Second message");
        let msg3 = SmsMessage::new("+15551111111", "Third message");

        sender.send_sms(&msg1).await.unwrap();
        sender.send_sms(&msg2).await.unwrap();
        sender.send_sms(&msg3).await.unwrap();

        let last = sender.last_message_for("+15551111111").await;
        assert!(last.is_some());
        assert_eq!(last.unwrap().body, "Third message");

        let last2 = sender.last_message_for("+15552222222").await;
        assert!(last2.is_some());
        assert_eq!(last2.unwrap().body, "Second message");

        let none = sender.last_message_for("+15559999999").await;
        assert!(none.is_none());
    }

    #[tokio::test]
    async fn test_memory_sms_sender_clear() {
        let sender = MemorySmsSender::new();
        let msg = SmsMessage::new("+15551234567", "Will be cleared");

        sender.send_sms(&msg).await.unwrap();
        assert_eq!(sender.sent_messages().await.len(), 1);

        sender.clear().await;
        assert!(sender.sent_messages().await.is_empty());
    }

    #[tokio::test]
    async fn test_memory_sms_sender_multiple_sends() {
        let sender = MemorySmsSender::new();

        for i in 0..5 {
            let msg = SmsMessage::new("+15551234567", format!("Message {i}"));
            sender.send_sms(&msg).await.unwrap();
        }

        let sent = sender.sent_messages().await;
        assert_eq!(sent.len(), 5);
    }
}
