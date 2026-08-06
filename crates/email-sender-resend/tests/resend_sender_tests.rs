use email_sender_resend::{ResendConfig, ResendEmailSender};

#[test]
fn test_resend_sender_creation() {
    let config = ResendConfig::new("re_test_key");
    let _sender = ResendEmailSender::new(config);
}
