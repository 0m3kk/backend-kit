use email_sender::{Attachment, Email, EmailAddress, EmailError, EmailResponse};

#[test]
fn test_email_address_creation_and_formatting() -> Result<(), EmailError> {
    let plain = EmailAddress::new("dev@backend-kit.com")?;
    assert_eq!(plain.email, "dev@backend-kit.com");
    assert_eq!(plain.name, None);
    assert_eq!(plain.to_string(), "dev@backend-kit.com");

    let with_name = EmailAddress::with_name("Backend Kit Developer", "dev@backend-kit.com")?;
    assert_eq!(with_name.email, "dev@backend-kit.com");
    assert_eq!(with_name.name.as_deref(), Some("Backend Kit Developer"));
    assert_eq!(
        with_name.to_string(),
        "Backend Kit Developer <dev@backend-kit.com>"
    );

    let empty_name = EmailAddress::with_name("   ", "dev@backend-kit.com")?;
    assert_eq!(empty_name.name, None);
    assert_eq!(empty_name.to_string(), "dev@backend-kit.com");

    Ok(())
}

#[test]
fn test_email_address_from_str_parsing() -> Result<(), EmailError> {
    let addr: EmailAddress = "alice@example.com".parse()?;
    assert_eq!(addr.email, "alice@example.com");
    assert_eq!(addr.name, None);

    let addr_formatted: EmailAddress = "Alice Smith <alice@example.com>".parse()?;
    assert_eq!(addr_formatted.name.as_deref(), Some("Alice Smith"));
    assert_eq!(addr_formatted.email, "alice@example.com");

    let addr_quoted: EmailAddress = "\"Bob Builder\" <bob@example.com>".parse()?;
    assert_eq!(addr_quoted.name.as_deref(), Some("Bob Builder"));
    assert_eq!(addr_quoted.email, "bob@example.com");

    Ok(())
}

#[test]
fn test_email_address_validation_errors() {
    assert!(matches!(
        EmailAddress::new(""),
        Err(EmailError::InvalidAddress(_))
    ));
    assert!(matches!(
        EmailAddress::new("invalid_no_at_sign"),
        Err(EmailError::InvalidAddress(_))
    ));
    assert!(matches!(
        EmailAddress::new("missingdomain@"),
        Err(EmailError::InvalidAddress(_))
    ));
    assert!(matches!(
        EmailAddress::new("@missinguser.com"),
        Err(EmailError::InvalidAddress(_))
    ));
    assert!(matches!(
        EmailAddress::new("user@nodotdomain"),
        Err(EmailError::InvalidAddress(_))
    ));
}

#[test]
fn test_attachment_creation() {
    let content = b"Sample text file content".to_vec();
    let att = Attachment::new("document.txt", content.clone(), "text/plain");

    assert_eq!(att.filename, "document.txt");
    assert_eq!(att.content, content);
    assert_eq!(att.content_type, "text/plain");
}

#[test]
fn test_email_builder_full() -> Result<(), EmailError> {
    let from = EmailAddress::with_name("Support Team", "support@example.com")?;
    let to1 = EmailAddress::new("client1@example.com")?;
    let to2 = EmailAddress::with_name("Client Two", "client2@example.com")?;
    let cc = EmailAddress::new("manager@example.com")?;
    let bcc = EmailAddress::new("audit@example.com")?;
    let reply_to = EmailAddress::new("help@example.com")?;

    let attachment = Attachment::new("invoice.pdf", b"%PDF-1.4...".to_vec(), "application/pdf");

    let email = Email::builder()
        .from(from.clone())
        .to(to1.clone())
        .to(to2.clone())
        .cc(cc.clone())
        .bcc(bcc.clone())
        .reply_to(reply_to.clone())
        .subject("Invoice #1042")
        .text_body("Please find attached your invoice.")
        .html_body("<h1>Invoice #1042</h1><p>Please find attached your invoice.</p>")
        .attach(attachment.clone())
        .header("X-Priority", "1")
        .header("X-Custom-ID", "INV-1042")
        .build()?;

    assert_eq!(email.from, from);
    assert_eq!(email.to, vec![to1, to2]);
    assert_eq!(email.cc, vec![cc]);
    assert_eq!(email.bcc, vec![bcc]);
    assert_eq!(email.reply_to, Some(reply_to));
    assert_eq!(email.subject, "Invoice #1042");
    assert_eq!(
        email.text_body.as_deref(),
        Some("Please find attached your invoice.")
    );
    assert_eq!(
        email.html_body.as_deref(),
        Some("<h1>Invoice #1042</h1><p>Please find attached your invoice.</p>")
    );
    assert_eq!(email.attachments, vec![attachment]);
    assert_eq!(
        email.headers.get("X-Priority").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        email.headers.get("X-Custom-ID").map(String::as_str),
        Some("INV-1042")
    );

    Ok(())
}

#[test]
fn test_email_builder_helper_string_methods() -> Result<(), EmailError> {
    let email = Email::builder()
        .from_str("Sender <sender@example.com>")?
        .to_str("Receiver <receiver@example.com>")?
        .subject("Test Subject")
        .text_body("Test Body")
        .build()?;

    assert_eq!(email.from.to_string(), "Sender <sender@example.com>");
    assert_eq!(email.to[0].to_string(), "Receiver <receiver@example.com>");
    assert_eq!(email.subject, "Test Subject");

    Ok(())
}

#[test]
fn test_email_builder_missing_from() -> Result<(), EmailError> {
    let to = EmailAddress::new("to@example.com")?;
    let err = Email::builder()
        .to(to)
        .subject("No sender")
        .text_body("Body")
        .build();

    assert_eq!(err.unwrap_err(), EmailError::MissingSender);
    Ok(())
}

#[test]
fn test_email_builder_missing_recipient() -> Result<(), EmailError> {
    let from = EmailAddress::new("from@example.com")?;
    let err = Email::builder()
        .from(from)
        .subject("No recipient")
        .text_body("Body")
        .build();

    assert_eq!(err.unwrap_err(), EmailError::MissingRecipient);
    Ok(())
}

#[test]
fn test_email_builder_missing_body() -> Result<(), EmailError> {
    let from = EmailAddress::new("from@example.com")?;
    let to = EmailAddress::new("to@example.com")?;
    let err = Email::builder()
        .from(from)
        .to(to)
        .subject("No body")
        .build();

    assert_eq!(err.unwrap_err(), EmailError::MissingContent);
    Ok(())
}

#[test]
fn test_email_response() {
    let res = EmailResponse::new(Some("msg-id-999".to_string()), Some("200 OK".to_string()));
    assert_eq!(res.message_id.as_deref(), Some("msg-id-999"));
    assert_eq!(res.provider_response.as_deref(), Some("200 OK"));
}
