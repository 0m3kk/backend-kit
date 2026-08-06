use email_sender::EmailError;
use email_sender_smtp::{SmtpConfig, SmtpEmailSender, SmtpTlsMode};

#[test]
fn test_smtp_sender_creation() -> Result<(), EmailError> {
    let config = SmtpConfig::new("127.0.0.1")
        .port(1025)
        .tls_mode(SmtpTlsMode::None);

    let sender = SmtpEmailSender::new(&config);
    assert!(sender.is_ok());
    Ok(())
}
