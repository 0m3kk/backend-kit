use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum KvError {
    #[error("Precondition unfulfilled (e.g. key exists or does not exist for NX/XX condition)")]
    ConditionFailed,

    #[error("Key not found")]
    KeyNotFound,

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Data corruption detected: {0}")]
    CorruptionDetected(String),

    #[error("Key-Value store error: {0}")]
    StoreError(String),
}
