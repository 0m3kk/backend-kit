use async_trait::async_trait;
use event_sourcing::SequencePosition;
use std::sync::Arc;
use thiserror::Error;

pub mod kv;
pub use kv::{KvCheckpointStore, KvCheckpointStoreTx};

/// Error type for CheckpointStore operations.
#[derive(Debug, Error)]
pub enum CheckpointError {
    #[error("Checkpoint store error: {0}")]
    Store(String),

    #[error("Parse error: {0}")]
    Parse(String),
}

/// Abstract storage interface for tracking projection progress positions for views.
#[async_trait]
pub trait CheckpointStore: Send + Sync {
    /// Retrieve the last processed position for a view table name.
    async fn get_position(
        &self,
        view_name: &str,
    ) -> Result<Option<SequencePosition>, CheckpointError>;

    /// Save the newly committed position for a view table name.
    async fn save_position(
        &self,
        view_name: &str,
        position: SequencePosition,
    ) -> Result<(), CheckpointError>;
}

#[async_trait]
impl<T: CheckpointStore + ?Sized> CheckpointStore for Arc<T> {
    async fn get_position(
        &self,
        view_name: &str,
    ) -> Result<Option<SequencePosition>, CheckpointError> {
        (**self).get_position(view_name).await
    }

    async fn save_position(
        &self,
        view_name: &str,
        position: SequencePosition,
    ) -> Result<(), CheckpointError> {
        (**self).save_position(view_name, position).await
    }
}

/// Transactional checkpoint storage interface participating in an active transaction handle `conn: &mut Conn`.
#[async_trait]
pub trait CheckpointStoreTx<Conn: Send>: Send + Sync {
    /// Retrieve the last processed position for a view within an active transaction.
    async fn get_position_tx(
        &self,
        conn: &mut Conn,
        view_name: &str,
    ) -> Result<Option<SequencePosition>, CheckpointError>;

    /// Save the newly committed position for a view within an active transaction.
    async fn save_position_tx(
        &self,
        conn: &mut Conn,
        view_name: &str,
        position: SequencePosition,
    ) -> Result<(), CheckpointError>;
}

#[async_trait]
impl<T: CheckpointStoreTx<Conn> + ?Sized, Conn: Send> CheckpointStoreTx<Conn> for Arc<T> {
    async fn get_position_tx(
        &self,
        conn: &mut Conn,
        view_name: &str,
    ) -> Result<Option<SequencePosition>, CheckpointError> {
        (**self).get_position_tx(conn, view_name).await
    }

    async fn save_position_tx(
        &self,
        conn: &mut Conn,
        view_name: &str,
        position: SequencePosition,
    ) -> Result<(), CheckpointError> {
        (**self).save_position_tx(conn, view_name, position).await
    }
}
