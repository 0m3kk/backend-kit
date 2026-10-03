use std::collections::BTreeMap;
use std::sync::Arc;

use crate::cipher::Cipher;

/// Version identifier for a key in a [`Keyring`].
pub type KeyVersion = u32;

/// A keyring managing an active cipher version for encryption and historical
/// cipher versions for decryption.
#[derive(Clone)]
pub struct Keyring {
    active_version: KeyVersion,
    active_cipher: Arc<dyn Cipher>,
    historical: BTreeMap<KeyVersion, Arc<dyn Cipher>>,
}

impl std::fmt::Debug for Keyring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keyring")
            .field("active_version", &self.active_version)
            .field(
                "historical_versions",
                &self.historical.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl Keyring {
    /// Create a new keyring with an active version and its cipher.
    pub fn new(active_version: KeyVersion, cipher: impl Cipher + 'static) -> Self {
        Self {
            active_version,
            active_cipher: Arc::new(cipher),
            historical: BTreeMap::new(),
        }
    }

    /// Create a new keyring from an `Arc<dyn Cipher>`.
    pub fn from_arc(active_version: KeyVersion, cipher: Arc<dyn Cipher>) -> Self {
        Self {
            active_version,
            active_cipher: cipher,
            historical: BTreeMap::new(),
        }
    }

    /// Add a historical cipher for decrypting secrets encrypted with older key versions.
    pub fn with_historical(mut self, version: KeyVersion, cipher: impl Cipher + 'static) -> Self {
        self.historical.insert(version, Arc::new(cipher));
        self
    }

    /// Add a historical cipher from an `Arc<dyn Cipher>`.
    pub fn with_historical_arc(mut self, version: KeyVersion, cipher: Arc<dyn Cipher>) -> Self {
        self.historical.insert(version, cipher);
        self
    }

    /// Returns the active key version.
    pub fn active_version(&self) -> KeyVersion {
        self.active_version
    }

    /// Returns the active cipher used for new encryptions.
    pub fn active_cipher(&self) -> &dyn Cipher {
        &*self.active_cipher
    }

    /// Get the cipher corresponding to the given key version.
    pub fn get(&self, version: KeyVersion) -> Option<&dyn Cipher> {
        if version == self.active_version {
            Some(&*self.active_cipher)
        } else {
            self.historical.get(&version).map(|c| &**c)
        }
    }
}
