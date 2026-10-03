/// Error returned when the [put](crate::KvStore::put) condition is not met.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PutConditionError {
    /// The current value of the key, if any, that prevented the put.
    pub current: Option<serde_json::Value>,
}

/// Error returned by [`KvStore`](crate::KvStore).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error<E> {
    /// The put condition was not met.
    Condition(PutConditionError),
    /// An error from the underlying store.
    Store(E),
}
