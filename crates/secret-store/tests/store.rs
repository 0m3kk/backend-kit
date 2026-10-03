//! Integration test for the SecretStore trait and MockSecretStore.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use secret_store::{Aes256GcmCipher, Key, Keyring, MockSecretStore, SecretBytes, SecretStore};

fn make_keyring() -> Keyring {
    let cipher = Aes256GcmCipher::new(Key::from_bytes([1u8; 32]));
    Keyring::new(1, cipher)
}

#[tokio::test]
async fn mock_secret_store_get_and_put() {
    let mut mock = MockSecretStore::new();
    mock.expect_get()
        .with(mockall::predicate::always(), mockall::predicate::eq("k"))
        .returning(|_, _| Ok(Some(SecretBytes::from_utf8("secret_val"))));

    let keyring = make_keyring();
    let result = mock.get(&keyring, "k").await.unwrap();
    assert_eq!(result.unwrap().as_bytes(), b"secret_val");

    mock.expect_put()
        .with(
            mockall::predicate::always(),
            mockall::predicate::eq("k"),
            mockall::predicate::always(),
        )
        .returning(|_, _, _| Ok(()));

    mock.put(&keyring, "k", &SecretBytes::from_utf8("secret_val"))
        .await
        .unwrap();
}

#[tokio::test]
async fn mock_secret_store_delete() {
    let mut mock = MockSecretStore::new();
    mock.expect_delete()
        .with(mockall::predicate::eq("k"))
        .returning(|_| Ok(true));

    let result = mock.delete("k").await.unwrap();
    assert!(result);
}
