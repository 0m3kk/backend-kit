use serde_json::Value;
use time::OffsetDateTime;

/// Timestamp of a store operation, in UTC.
pub type Timestamp = OffsetDateTime;

/// A value stored under a key, with the key it was stored under.
///
/// The key is duplicated into the returned value so a multi-key operation can
/// be matched against the request without keeping a map around.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredValue {
    /// The key the value is stored under.
    pub key: String,
    /// Opaque JSON payload of the value.
    pub value: Value,
    /// When the key was first created. `None` when the store cannot tell.
    pub created_at: Option<Timestamp>,
    /// When the value was last written. `None` when the store cannot tell.
    pub updated_at: Option<Timestamp>,
}

/// A value read from the store, with the previous value it replaced.
///
/// A `previous` of `None` means the key did not exist before.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacedValue {
    /// The key the value is stored under.
    pub key: String,
    /// The value that was stored.
    pub value: Value,
    /// The value that was previously stored under the key.
    pub previous: Option<Value>,
    /// When the key was first created. `None` when the store cannot tell.
    pub created_at: Option<Timestamp>,
    /// When the value was last written. `None` when the store cannot tell.
    pub updated_at: Option<Timestamp>,
}
