use email_sender::{Email, EmailAddress, EmailError, EmailSender};
use email_sender_memory::MemoryEmailSender;

#[tokio::test]
async fn test_batch_sending_memory() -> Result<(), EmailError> {
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
