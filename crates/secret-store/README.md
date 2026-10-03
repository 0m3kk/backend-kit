# 🔐 secret-store

Production-ready, secure **Envelope Encryption** Secret Store for Rust applications.

---

## ✨ Features

- 🔒 **Envelope Encryption**: Per-secret Data Encryption Key (DEK) encrypted with a Master Key Encryption Key (KEK) using AES-256-GCM.
- 🔄 **Zero-Downtime Key Rotation**: Rotate KEKs seamlessly across database secrets via [`Keyring`].
- 🐘 **PostgreSQL Engine**: Built-in [`PostgresSecretStore`] for high-performance persistence and concurrency control.
- 🛡️ **Zeroize Protection**: Memory zeroization using `zeroize` to prevent sensitive key material leakage.
- 📊 **OpenTelemetry Built-in**: Out-of-the-box metrics (`puts_total`, `put_duration_seconds`, `gets_total`, `rotations_total`, etc.).
- 🧪 **Mocking Ready**: Ships with [`MockSecretStore`] generated via `mockall` for seamless unit testing.

---

## 🚀 Quick Start

### 1. Add Dependency

Add `secret-store` to your `Cargo.toml`:

```toml
[dependencies]
secret-store = "0.1"
```

### 2. Usage Example

```rust
use secret_store::{
    Aes256GcmCipher, Key, Keyring, PostgresSecretStore, SECRET_STORE_MIGRATOR, SecretBytes,
    SecretStore,
};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Connect to PostgreSQL
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://postgres:password@localhost:5432/secret_store_db")
        .await?;

    // 2. Run embedded migrations automatically
    SECRET_STORE_MIGRATOR.run(&pool).await?;

    // 3. Setup Master Key (KEK) and Keyring
    let master_key = Key::<32>::from_bytes([0x42; 32]);
    let cipher = Aes256GcmCipher::new(master_key);
    let keyring = Keyring::new(1, cipher);

    // 4. Initialize Secret Store
    let store = PostgresSecretStore::new(pool);

    // 5. Store a Secret
    let secret = SecretBytes::from_utf8("super-secret-password");
    store.put(&keyring, "db/password", &secret).await?;

    // 6. Retrieve & Decrypt Secret
    if let Some(loaded) = store.get(&keyring, "db/password").await? {
        println!("Decrypted {} bytes", loaded.len());
    }

    // 7. Zero-Downtime Key Rotation
    let new_key = Key::<32>::from_bytes([0x43; 32]);
    let new_cipher = Aes256GcmCipher::new(new_key);
    let rotated_keyring = Keyring::new(2, new_cipher).with_historical_arc(1, keyring.active_cipher().clone());
    let count = store.rotate_keys(&rotated_keyring).await?;
    println!("Rotated {count} secrets to new KEK version");

    // 8. Delete Secret
    store.delete("db/password").await?;

    Ok(())
}
```

---

## 🧪 Unit Testing with Mock

[`MockSecretStore`] allows testing domain logic without spinning up a real database:

```rust
use secret_store::{MockSecretStore, SecretBytes, SecretStore};

#[tokio::test]
async fn test_service_with_mock_store() {
    let mut mock = MockSecretStore::new();
    mock.expect_get()
        .returning(|_, _| Ok(Some(SecretBytes::from_utf8("test-secret"))));

    // Inject `mock` into your service handlers
}
```

---

## 🗄️ Database Schema

The PostgreSQL table schema includes descriptions on all columns:

| Column | Type | Description |
| :--- | :--- | :--- |
| 🔑 `name` | `TEXT` | Primary Key. Unique string identifier for the secret (e.g. `db/password`). |
| 🔐 `payload_ciphertext` | `BYTEA` | Encrypted payload data (DEK encrypted). |
| 🎲 `payload_nonce` | `BYTEA` | Nonce used for payload encryption. |
| 🔑 `encrypted_dek` | `BYTEA` | Data Encryption Key (DEK) encrypted with KEK. |
| 🎲 `dek_nonce` | `BYTEA` | Nonce used for DEK encryption. |
| 🧂 `dek_salt` | `BYTEA` | Optional salt for key derivation. |
| 🏷️ `kek_version` | `INTEGER` | Active KEK version used to encrypt the DEK. Indexed for fast key rotation. |
| ⏰ `created_at` | `TIMESTAMPTZ` | Secret creation timestamp. |
| 🔄 `updated_at` | `TIMESTAMPTZ` | Secret last update timestamp. |

---

## 📜 License

Licensed under the [MIT License](LICENSE).
