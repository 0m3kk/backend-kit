pub mod errors;
#[cfg(feature = "memory")]
pub mod memory;
pub mod sender;
pub mod types;

pub use errors::EmailError;
pub use sender::EmailSender;
pub use types::{Attachment, Email, EmailAddress, EmailBuilder, EmailResponse};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_address_parsing() {
        let addr = EmailAddress::new("alice@example.com");
        assert!(addr.is_ok());
        let addr = addr.unwrap();
        assert_eq!(addr.email, "alice@example.com");
        assert_eq!(addr.name, None);
        assert_eq!(addr.to_string(), "alice@example.com");

        let addr_with_name = EmailAddress::with_name("Alice", "alice@example.com");
        assert!(addr_with_name.is_ok());
        let addr_with_name = addr_with_name.unwrap();
        assert_eq!(addr_with_name.name.as_deref(), Some("Alice"));
        assert_eq!(addr_with_name.to_string(), "Alice <alice@example.com>");

        let parsed: Result<EmailAddress, EmailError> = "Bob <bob@example.com>".parse();
        assert!(parsed.is_ok());
        let parsed = parsed.unwrap();
        assert_eq!(parsed.name.as_deref(), Some("Bob"));
        assert_eq!(parsed.email, "bob@example.com");
    }

    #[test]
    fn test_invalid_email_address() {
        assert!(EmailAddress::new("invalid-email").is_err());
        assert!(EmailAddress::new("user@").is_err());
        assert!(EmailAddress::new("@domain.com").is_err());
        assert!(EmailAddress::new("user@domain").is_err());
    }

    #[test]
    fn test_email_builder() {
        let from = EmailAddress::new("sender@example.com").unwrap();
        let to = EmailAddress::new("receiver@example.com").unwrap();

        let email = Email::builder()
            .from(from.clone())
            .to(to.clone())
            .subject("Hello")
            .text_body("Text body")
            .html_body("<p>HTML body</p>")
            .attach(Attachment::new(
                "doc.txt",
                b"hello world".to_vec(),
                "text/plain",
            ))
            .build();

        assert!(email.is_ok());
        let email = email.unwrap();
        assert_eq!(email.from, from);
        assert_eq!(email.to, vec![to]);
        assert_eq!(email.subject, "Hello");
        assert_eq!(email.text_body.as_deref(), Some("Text body"));
        assert_eq!(email.html_body.as_deref(), Some("<p>HTML body</p>"));
        assert_eq!(email.attachments.len(), 1);
    }

    #[test]
    fn test_email_builder_validation() {
        let from = EmailAddress::new("sender@example.com").unwrap();

        // Missing recipient
        let err = Email::builder()
            .from(from.clone())
            .subject("Test")
            .text_body("Hello")
            .build();
        assert_eq!(err.unwrap_err(), EmailError::MissingRecipient);

        // Missing body
        let to = EmailAddress::new("receiver@example.com").unwrap();
        let err = Email::builder().from(from).to(to).subject("Test").build();
        assert_eq!(err.unwrap_err(), EmailError::MissingContent);
    }
}
