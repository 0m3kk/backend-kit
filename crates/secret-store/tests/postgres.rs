//! Behavior suite for the postgres secret store, run against a real database
//! via `#[sqlx::test]`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use secret_store::{
    Cipher, CryptoError, EncryptedData, Error, Keyring, PostgresSecretStore, SecretBytes,
    SecretStore,
};
use sqlx::PgPool;

struct MarkerCipher {
    marker: u8,
}

impl Cipher for MarkerCipher {
    fn encrypt(&self, plain: &SecretBytes) -> Result<EncryptedData, CryptoError> {
        Ok(EncryptedData {
            ciphertext: [vec![self.marker], plain.as_bytes().to_vec()].concat(),
            nonce: [self.marker; 12],
            salt: None,
        })
    }

    fn decrypt(&self, enc: &EncryptedData) -> Result<SecretBytes, CryptoError> {
        if enc.ciphertext.first() == Some(&self.marker) {
            Ok(SecretBytes::from_bytes(enc.ciphertext[1..].to_vec()))
        } else {
            Err(CryptoError::Decrypt)
        }
    }
}

#[sqlx::test]
async fn get_missing_name_returns_none(pool: PgPool) {
    let store = PostgresSecretStore::new(pool);
    let keyring = Keyring::new(1, MarkerCipher { marker: 1 });

    let result = store.get(&keyring, "missing").await.unwrap();
    assert!(result.is_none());
}

#[sqlx::test]
async fn put_then_get_roundtrips(pool: PgPool) {
    let store = PostgresSecretStore::new(pool);
    let keyring = Keyring::new(1, MarkerCipher { marker: 1 });

    store
        .put(&keyring, "db/password", &SecretBytes::from_utf8("hunter2"))
        .await
        .unwrap();

    let result = store.get(&keyring, "db/password").await.unwrap().unwrap();
    assert_eq!(result.as_bytes(), b"hunter2");
}

#[sqlx::test]
async fn put_overwrites(pool: PgPool) {
    let store = PostgresSecretStore::new(pool);
    let keyring = Keyring::new(1, MarkerCipher { marker: 1 });

    store
        .put(&keyring, "k", &SecretBytes::from_utf8("v1"))
        .await
        .unwrap();
    store
        .put(&keyring, "k", &SecretBytes::from_utf8("v2"))
        .await
        .unwrap();

    let result = store.get(&keyring, "k").await.unwrap().unwrap();
    assert_eq!(result.as_bytes(), b"v2");
}

#[sqlx::test]
async fn delete_returns_whether_it_existed(pool: PgPool) {
    let store = PostgresSecretStore::new(pool);
    let keyring = Keyring::new(1, MarkerCipher { marker: 1 });

    assert!(!store.delete("k").await.unwrap());

    store
        .put(&keyring, "k", &SecretBytes::from_utf8("v"))
        .await
        .unwrap();

    assert!(store.delete("k").await.unwrap());
    assert!(store.get(&keyring, "k").await.unwrap().is_none());
}

#[sqlx::test]
async fn list_names_returns_names_only(pool: PgPool) {
    let store = PostgresSecretStore::new(pool);
    let keyring = Keyring::new(1, MarkerCipher { marker: 1 });

    store
        .put(&keyring, "b", &SecretBytes::from_utf8("v"))
        .await
        .unwrap();
    store
        .put(&keyring, "a", &SecretBytes::from_utf8("v"))
        .await
        .unwrap();

    let names = store.list_names().await.unwrap();
    assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
}

#[sqlx::test]
async fn rotate_keys_reencrypts_all_secrets(pool: PgPool) {
    let store = PostgresSecretStore::new(pool);
    let v1_keyring = Keyring::new(1, MarkerCipher { marker: 1 });

    store
        .put(&v1_keyring, "a", &SecretBytes::from_utf8("va"))
        .await
        .unwrap();
    store
        .put(&v1_keyring, "b", &SecretBytes::from_utf8("vb"))
        .await
        .unwrap();
    store
        .put(&v1_keyring, "c", &SecretBytes::from_utf8("vc"))
        .await
        .unwrap();

    let v2_keyring =
        Keyring::new(2, MarkerCipher { marker: 2 }).with_historical(1, MarkerCipher { marker: 1 });

    let count = store.rotate_keys(&v2_keyring).await.unwrap();
    assert_eq!(count, 3);

    for (name, value) in [("a", "va"), ("b", "vb"), ("c", "vc")] {
        let result = store.get(&v2_keyring, name).await.unwrap().unwrap();
        assert_eq!(result.as_bytes(), value.as_bytes());
    }
}

#[sqlx::test]
async fn rotate_keys_breaks_old_cipher(pool: PgPool) {
    let store = PostgresSecretStore::new(pool);
    let v1_keyring = Keyring::new(1, MarkerCipher { marker: 1 });

    store
        .put(&v1_keyring, "k", &SecretBytes::from_utf8("v"))
        .await
        .unwrap();

    let v2_keyring =
        Keyring::new(2, MarkerCipher { marker: 2 }).with_historical(1, MarkerCipher { marker: 1 });
    store.rotate_keys(&v2_keyring).await.unwrap();

    let result = store.get(&v1_keyring, "k").await;
    assert!(matches!(result, Err(Error::MissingKeyVersion { .. })));
}

#[sqlx::test]
async fn rotate_keys_on_empty_store_returns_zero(pool: PgPool) {
    let store = PostgresSecretStore::new(pool);
    let v2_keyring =
        Keyring::new(2, MarkerCipher { marker: 2 }).with_historical(1, MarkerCipher { marker: 1 });

    let count = store.rotate_keys(&v2_keyring).await.unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test]
async fn raw_row_fails_decryption(pool: PgPool) {
    let store = PostgresSecretStore::new(pool.clone());
    let keyring = Keyring::new(1, MarkerCipher { marker: 1 });

    sqlx::query(
        "INSERT INTO secrets (name, payload_ciphertext, payload_nonce, encrypted_dek, dek_nonce, kek_version) \
         VALUES ('raw', '\\xdeadbeef', '\\x000000000000000000000000', '\\xdeadbeef', '\\x000000000000000000000000', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let result = store.get(&keyring, "raw").await;
    match result {
        Err(Error::Decrypt(error)) => assert_eq!(error.name, "raw"),
        other => panic!("expected decrypt error, got {other:?}"),
    }
}
