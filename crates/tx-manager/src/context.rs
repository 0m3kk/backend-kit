use async_trait::async_trait;

/// Abstract transactional context interface managing transaction commit and rollback lifecycle.
#[async_trait]
pub trait TxContext: Send {
    /// Storage error type produced during transaction commit or rollback.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Commit all changes performed within this transactional context atomically.
    async fn commit(self) -> Result<(), Self::Error>;

    /// Explicitly rollback and abort all changes performed within this transactional context.
    async fn rollback(self) -> Result<(), Self::Error>;
}
