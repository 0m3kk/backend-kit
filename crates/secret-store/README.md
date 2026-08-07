# secret-store

Core specification, cryptographic primitives, envelope encryption, and `SecretStore` trait for backend-kit.

## Feature Flags

| Feature  | Description                                                           | Default |
| :------- | :-------------------------------------------------------------------- | :------ |
| `memory` | In-memory `MemorySecretStore` with envelope encryption & key rotation | No      |

## Features

- **Envelope Encryption (DEK + KEK)**: Every secret payload is sealed under a fresh random Data Encryption Key (DEK). Each DEK is wrapped under a versioned Master Key (KEK) from a `KeyRing`.
- **Authenticated AEAD Ciphers**: Support for `Aes256Gcm` and `ChaCha20Poly1305`.
- **`KeyRing` & Versioned Master Keys**: Numeric key versions (`u32`) managed via `KeyRing`. Automatically uses the highest version for writes while keeping older versions readable for seamless key rotation.
- **Memory Safety & Zeroization**: `SecretValue` wrapper redacts sensitive material in `Debug` output (`[REDACTED]`) and zeroizes memory buffers on drop. `MasterKey` redacts key bytes in `Debug` output.
- **Hierarchical Pathing**: `SecretPath` wrapper for structured secret names (e.g., `prod/database/password`).
- **Immutable Secret Versioning**: Explicit tracking and retrieval of specific immutable versions per path.
- **Metadata & Tagging**: Associate custom key-value tags with secrets and query headers without exposing secrets.
- **Transactional API (`SecretStoreTx<Conn>`)**: All operations available as `_tx` methods accepting an external connection handle. Caller owns the transaction lifecycle.

## Code Example

```rust
use std::time::Duration;
use secret_store::{
    CipherAlgorithm, KeyRing, MasterKey, SecretPath, SecretStore, SecretValue, SetSecretOptions, KEY_LEN,
};
use secret_store::memory::MemorySecretStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create a versioned MasterKey and KeyRing
    let master_key = MasterKey::new(1, [42u8; KEY_LEN]);
    let keyring = KeyRing::new([master_key])?;

    // 2. Initialize the Secret Store
    let store = MemorySecretStore::new(keyring, CipherAlgorithm::Aes256Gcm);

    // 3. Store an encrypted secret
    let path = SecretPath::new("prod/db/password")?;
    let secret = SecretValue::from("super_secret_db_pass");
    store.set(path.clone(), secret, SetSecretOptions::new().with_tag("env", "prod")).await?;

    // 4. Retrieve latest version
    if let Some(entry) = store.get(&path).await? {
        println!("Path: {}, Version: {}, Secret: {}", entry.path, entry.version, entry.value.as_str()?);
    }

    Ok(())
}
```
