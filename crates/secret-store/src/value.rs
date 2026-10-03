use zeroize::Zeroizing;

/// A byte buffer whose memory is zeroized on drop.
#[derive(Clone)]
pub struct SecretBytes {
    bytes: Zeroizing<Vec<u8>>,
}

impl SecretBytes {
    /// Wrap raw bytes as a zeroizing buffer.
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: Zeroizing::new(bytes.into()),
        }
    }

    /// Wrap a string as a zeroizing UTF-8 buffer.
    pub fn from_utf8(s: impl Into<String>) -> Self {
        Self::from_bytes(s.into().into_bytes())
    }

    /// The underlying bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Number of bytes.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl From<&str> for SecretBytes {
    fn from(s: &str) -> Self {
        Self::from_utf8(s)
    }
}

impl From<String> for SecretBytes {
    fn from(s: String) -> Self {
        Self::from_utf8(s)
    }
}

impl std::fmt::Debug for SecretBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretBytes([REDACTED])")
    }
}

/// A secret value alias for [`SecretBytes`].
pub type SecretValue = SecretBytes;

/// Opaque ciphertext produced by [`Cipher::encrypt`](crate::Cipher::encrypt).
#[derive(Clone)]
pub struct EncryptedData {
    /// The authenticated ciphertext.
    pub ciphertext: Vec<u8>,
    /// The 96-bit nonce used for this encryption.
    pub nonce: [u8; 12],
    /// The KDF salt, present only when the key was derived from a passphrase.
    pub salt: Option<[u8; 16]>,
}

impl std::fmt::Debug for EncryptedData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptedData")
            .field("ciphertext_len", &self.ciphertext.len())
            .field("has_salt", &self.salt.is_some())
            .finish_non_exhaustive()
    }
}
