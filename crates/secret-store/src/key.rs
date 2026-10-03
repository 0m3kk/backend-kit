use zeroize::Zeroizing;

use crate::cipher::CryptoError;
use crate::value::SecretBytes;

/// Key material of exactly `N` bytes, zeroized on drop.
#[derive(Clone)]
pub struct Key<const N: usize> {
    bytes: Zeroizing<[u8; N]>,
}

impl<const N: usize> Key<N> {
    /// Wrap raw key material.
    pub fn from_bytes(bytes: [u8; N]) -> Self {
        Self {
            bytes: Zeroizing::new(bytes),
        }
    }

    /// Decode a key from a hex (2N chars) or base64 (ceil(4N/3)) encoded string.
    pub fn from_str_encoded(s: &str) -> Result<Self, CryptoError> {
        let hex_len = N * 2;

        if s.len() == hex_len {
            let decoded = hex_decode(s).ok_or(CryptoError::InvalidEncodedKey)?;
            let mut bytes = [0u8; N];
            bytes.copy_from_slice(&decoded);
            return Ok(Self::from_bytes(bytes));
        }

        if is_base64_len(s.len(), N) {
            let decoded = base64_decode(s).ok_or(CryptoError::InvalidEncodedKey)?;
            if decoded.len() == N {
                let mut bytes = [0u8; N];
                bytes.copy_from_slice(&decoded);
                return Ok(Self::from_bytes(bytes));
            }
        }

        Err(CryptoError::InvalidEncodedKey)
    }

    /// The underlying key bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..]
    }
}

impl<const N: usize> std::fmt::Debug for Key<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Key([REDACTED])")
    }
}

/// Salt used for passphrase-derived keys.
pub struct Salt(pub [u8; 16]);

/// Derive a 256-bit key from a passphrase using argon2id.
pub fn derive_key_from_passphrase(
    passphrase: &SecretBytes,
    salt: &[u8; 16],
) -> Result<Key<32>, CryptoError> {
    use argon2::Algorithm;

    let params = argon2::Params::new(19 * 1024, 2, 1, Some(32)).map_err(CryptoError::Argon2)?;
    let argon2 = argon2::Argon2::new(Algorithm::Argon2id, argon2::Version::V0x13, params);

    let mut key = [0u8; 32];
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(CryptoError::Argon2)?;

    Ok(Key::from_bytes(key))
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for pair in bytes.chunks(2) {
        let hi = HEX
            .iter()
            .position(|&c| c == pair[0].to_ascii_lowercase())?;
        let lo = HEX
            .iter()
            .position(|&c| c == pair[1].to_ascii_lowercase())?;
        out.push((hi << 4 | lo) as u8);
    }
    Some(out)
}

fn is_base64_len(len: usize, key_len: usize) -> bool {
    let unpadded = 4 * key_len.div_ceil(3);
    len == unpadded || len == unpadded + (4 - key_len % 3) % 4
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for &b in bytes {
        if b == b'=' {
            break;
        }
        let val = B64.iter().position(|&c| c == b)?;
        acc = (acc << 6) | val as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}
