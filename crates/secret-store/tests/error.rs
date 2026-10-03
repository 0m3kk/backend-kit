//! Integration test for the error module.

use secret_store::{DecryptError, Error};

#[test]
fn store_error_is_constructible() {
    let error: Error<&str> = Error::Store("boom");
    assert!(matches!(error, Error::Store("boom")));
}

#[test]
fn decrypt_error_is_constructible() {
    let error: Error<&str> = Error::Decrypt(DecryptError {
        name: "test".to_string(),
    });

    match error {
        Error::Decrypt(decrypt_error) => {
            assert_eq!(decrypt_error.name, "test");
        }
        _ => panic!("expected decrypt error"),
    }
}

#[test]
fn missing_key_version_error_is_constructible() {
    let error: Error<&str> = Error::MissingKeyVersion {
        name: "test".to_string(),
        version: 2,
    };

    match error {
        Error::MissingKeyVersion { name, version } => {
            assert_eq!(name, "test");
            assert_eq!(version, 2);
        }
        _ => panic!("expected missing key version error"),
    }
}
