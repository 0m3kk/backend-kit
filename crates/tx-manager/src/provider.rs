use async_trait::async_trait;
use std::fmt::Display;
use std::sync::Arc;

/// Abstract provider interface for managing database transaction lifecycles (begin, commit, rollback)
/// directly without relying on manual caller coordination.
#[async_trait]
pub trait TransactionProvider: Send + Sync {
    type Conn: Send;
    type Error: Display + Send + Sync + 'static;

    /// Begin a new database transaction handle.
    async fn begin_tx(&self) -> Result<Self::Conn, Self::Error>;

    /// Commit the active database transaction handle.
    async fn commit_tx(&self, conn: Self::Conn) -> Result<(), Self::Error>;

    /// Rollback the active database transaction handle.
    async fn rollback_tx(&self, conn: Self::Conn) -> Result<(), Self::Error>;
}

#[async_trait]
impl<T: TransactionProvider + ?Sized> TransactionProvider for Arc<T> {
    type Conn = T::Conn;
    type Error = T::Error;

    async fn begin_tx(&self) -> Result<Self::Conn, Self::Error> {
        (**self).begin_tx().await
    }

    async fn commit_tx(&self, conn: Self::Conn) -> Result<(), Self::Error> {
        (**self).commit_tx(conn).await
    }

    async fn rollback_tx(&self, conn: Self::Conn) -> Result<(), Self::Error> {
        (**self).rollback_tx(conn).await
    }
}

#[async_trait]
impl TransactionProvider for sqlx::PgPool {
    type Conn = sqlx::Transaction<'static, sqlx::Postgres>;
    type Error = sqlx::Error;

    async fn begin_tx(&self) -> Result<Self::Conn, Self::Error> {
        self.begin().await
    }

    async fn commit_tx(&self, conn: Self::Conn) -> Result<(), Self::Error> {
        conn.commit().await
    }

    async fn rollback_tx(&self, conn: Self::Conn) -> Result<(), Self::Error> {
        conn.rollback().await
    }
}
