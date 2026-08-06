# secret-store

Core specification, cryptographic primitives, and `SecretStore` trait for backend-kit.

## Features

- **Authenticated Encryption at Rest**: Built-in support for `Aes256Gcm` and `ChaCha20Poly1305` AEAD ciphers.
- **Secret Value Safety**: `SecretValue` wrapper redacts sensitive content in debug output and zeroizes memory buffers on drop.
- **Hierarchical Pathing**: `SecretPath` wrapper for structured secret names (e.g. `prod/db/password`).
- **Secret Versioning**: Explicit tracking and retrieval of specific immutable versions.
- **Key Rotation Support**: Trait definition for re-encrypting stored ciphertext under new master keys.
- **Metadata & Tagging**: Associate custom key-value tags with secrets and search headers without exposing secrets.
