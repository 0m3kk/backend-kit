use async_trait::async_trait;

/// Abstract Unit of Work interface managing the transaction commit and rollback lifecycle.
#[async_trait]
pub trait UnitOfWork: Send {
    /// Storage error type produced during transaction commit or rollback.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Commit all changes performed within this Unit of Work atomically.
    async fn commit(self) -> Result<(), Self::Error>;

    /// Explicitly rollback and abort all changes performed within this Unit of Work.
    async fn rollback(self) -> Result<(), Self::Error>;
}
