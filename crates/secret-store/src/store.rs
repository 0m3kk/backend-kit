use async_trait::async_trait;
use std::sync::Arc;

use crate::errors::SecretError;
use crate::types::{
    ListSecretOptions, SecretEntry, SecretHeader, SecretPath, SecretValue, SetSecretOptions,
};

/// Universal Secret Store specification trait providing async secret management,
/// secret versioning, envelope encryption (DEK + KeyRing KEK), tag filtering, master key rotation, and TTL expiration.
#[async_trait]
pub trait SecretStore: Send + Sync {
    /// Retrieve the latest active version of a secret by path.
    /// Returns `None` if the secret does not exist, is marked deleted, or has expired.
    async fn get(&self, path: &SecretPath) -> Result<Option<SecretEntry>, SecretError>;

    /// Retrieve a specific version of a secret by path and version number.
    /// Returns `None` if that specific version does not exist.
    async fn get_version(
        &self,
        path: &SecretPath,
        version: u64,
    ) -> Result<Option<SecretEntry>, SecretError>;

    /// Store a new version of a secret at path. Automatically increments secret version.
    async fn set(
        &self,
        path: SecretPath,
        value: SecretValue,
        options: SetSecretOptions,
    ) -> Result<SecretEntry, SecretError>;

    /// Delete a secret at path. Returns `true` if the secret was found and deleted.
    async fn delete(&self, path: &SecretPath) -> Result<bool, SecretError>;

    /// List secret headers matching prefix/tag filters without returning secret values.
    async fn list(&self, options: ListSecretOptions) -> Result<Vec<SecretHeader>, SecretError>;

    /// Re-wrap secret entries stored under older master key versions to the current master key version.
    /// Returns the number of secret versions re-wrapped.
    async fn rotate_key(&self) -> Result<u64, SecretError>;

    /// Purge up to `limit` expired secrets from the store. Returns the number of purged secret versions.
    async fn clean_expired(&self, limit: Option<usize>) -> Result<u64, SecretError>;
}

#[async_trait]
impl<T: SecretStore + ?Sized> SecretStore for Arc<T> {
    async fn get(&self, path: &SecretPath) -> Result<Option<SecretEntry>, SecretError> {
        (**self).get(path).await
    }

    async fn get_version(
        &self,
        path: &SecretPath,
        version: u64,
    ) -> Result<Option<SecretEntry>, SecretError> {
        (**self).get_version(path, version).await
    }

    async fn set(
        &self,
        path: SecretPath,
        value: SecretValue,
        options: SetSecretOptions,
    ) -> Result<SecretEntry, SecretError> {
        (**self).set(path, value, options).await
    }

    async fn delete(&self, path: &SecretPath) -> Result<bool, SecretError> {
        (**self).delete(path).await
    }

    async fn list(&self, options: ListSecretOptions) -> Result<Vec<SecretHeader>, SecretError> {
        (**self).list(options).await
    }

    async fn rotate_key(&self) -> Result<u64, SecretError> {
        (**self).rotate_key().await
    }

    async fn clean_expired(&self, limit: Option<usize>) -> Result<u64, SecretError> {
        (**self).clean_expired(limit).await
    }
}

/// Transactional secret store operations.
///
/// `Conn` represents the connection or transaction handle type. The caller owns the
/// transaction lifecycle — the store only executes operations through the provided handle.
#[async_trait]
pub trait SecretStoreTx<Conn: Send>: SecretStore {
    /// Retrieve the latest active version of a secret using the provided connection handle.
    async fn get_tx(
        &self,
        conn: &mut Conn,
        path: &SecretPath,
    ) -> Result<Option<SecretEntry>, SecretError>;

    /// Retrieve a specific version of a secret using the provided connection handle.
    async fn get_version_tx(
        &self,
        conn: &mut Conn,
        path: &SecretPath,
        version: u64,
    ) -> Result<Option<SecretEntry>, SecretError>;

    /// Store a new version of a secret using the provided connection handle.
    async fn set_tx(
        &self,
        conn: &mut Conn,
        path: SecretPath,
        value: SecretValue,
        options: SetSecretOptions,
    ) -> Result<SecretEntry, SecretError>;

    /// Delete a secret using the provided connection handle.
    async fn delete_tx(&self, conn: &mut Conn, path: &SecretPath) -> Result<bool, SecretError>;

    /// List secret headers using the provided connection handle.
    async fn list_tx(
        &self,
        conn: &mut Conn,
        options: ListSecretOptions,
    ) -> Result<Vec<SecretHeader>, SecretError>;

    /// Re-wrap secret entries to the current master key version using the provided connection handle.
    async fn rotate_key_tx(&self, conn: &mut Conn) -> Result<u64, SecretError>;

    /// Purge up to `limit` expired secrets using the provided connection handle.
    async fn clean_expired_tx(
        &self,
        conn: &mut Conn,
        limit: Option<usize>,
    ) -> Result<u64, SecretError>;
}

#[async_trait]
impl<T: SecretStoreTx<Conn> + ?Sized, Conn: Send> SecretStoreTx<Conn> for Arc<T> {
    async fn get_tx(
        &self,
        conn: &mut Conn,
        path: &SecretPath,
    ) -> Result<Option<SecretEntry>, SecretError> {
        (**self).get_tx(conn, path).await
    }

    async fn get_version_tx(
        &self,
        conn: &mut Conn,
        path: &SecretPath,
        version: u64,
    ) -> Result<Option<SecretEntry>, SecretError> {
        (**self).get_version_tx(conn, path, version).await
    }

    async fn set_tx(
        &self,
        conn: &mut Conn,
        path: SecretPath,
        value: SecretValue,
        options: SetSecretOptions,
    ) -> Result<SecretEntry, SecretError> {
        (**self).set_tx(conn, path, value, options).await
    }

    async fn delete_tx(&self, conn: &mut Conn, path: &SecretPath) -> Result<bool, SecretError> {
        (**self).delete_tx(conn, path).await
    }

    async fn list_tx(
        &self,
        conn: &mut Conn,
        options: ListSecretOptions,
    ) -> Result<Vec<SecretHeader>, SecretError> {
        (**self).list_tx(conn, options).await
    }

    async fn rotate_key_tx(&self, conn: &mut Conn) -> Result<u64, SecretError> {
        (**self).rotate_key_tx(conn).await
    }

    async fn clean_expired_tx(
        &self,
        conn: &mut Conn,
        limit: Option<usize>,
    ) -> Result<u64, SecretError> {
        (**self).clean_expired_tx(conn, limit).await
    }
}
