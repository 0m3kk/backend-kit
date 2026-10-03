//! Secret store: types, [`SecretStore`] trait, [`Keyring`], envelope encryption, and PostgreSQL implementation.

mod cipher;
mod envelope;
mod error;
mod key;
mod keyring;
mod postgres;
mod store;
mod value;

pub use cipher::{Aes256GcmCipher, Cipher, CryptoError};
pub use envelope::{EnvelopeData, decrypt_envelope, encrypt_envelope, reencrypt_dek};
pub use error::{DecryptError, Error};
pub use key::{Key, Salt, derive_key_from_passphrase};
pub use keyring::{KeyVersion, Keyring};
pub use postgres::{PostgresSecretStore, SECRET_STORE_MIGRATOR};
pub use store::{MockSecretStore, SecretStore};
pub use value::{EncryptedData, SecretBytes, SecretValue};
