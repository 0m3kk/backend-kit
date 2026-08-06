use email_sender_sendgrid::{SendGridConfig, SendGridEmailSender};

#[test]
fn test_sendgrid_sender_creation() {
    let config = SendGridConfig::new("SG.test_key");
    let _sender = SendGridEmailSender::new(config);
}
