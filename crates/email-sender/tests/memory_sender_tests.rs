use email_sender::memory::MemoryEmailSender;
use email_sender::{Email, EmailAddress, EmailError, EmailSender};

#[tokio::test]
async fn test_captures_emails() -> Result<(), EmailError> {
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
async fn test_forced_error() -> Result<(), EmailError> {
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

#[tokio::test]
async fn test_batch_sending() -> Result<(), EmailError> {
    let sender = MemoryEmailSender::new();

    let email1 = Email::builder()
        .from(EmailAddress::new("sender@example.com")?)
        .to(EmailAddress::new("user1@example.com")?)
        .subject("Welcome User 1")
        .text_body("Hello 1")
        .build()?;

    let email2 = Email::builder()
        .from(EmailAddress::new("sender@example.com")?)
        .to(EmailAddress::new("user2@example.com")?)
        .subject("Welcome User 2")
        .text_body("Hello 2")
        .build()?;

    let results = sender.send_batch(&[email1, email2]).await?;
    assert_eq!(results.len(), 2);
    assert_eq!(sender.count().await, 2);

    let sent = sender.sent_emails().await;
    assert_eq!(sent[0].subject, "Welcome User 1");
    assert_eq!(sent[1].subject, "Welcome User 2");

    Ok(())
}
