use async_trait::async_trait;
use event_sourcing::{AppendCondition, AppendError, Event, EventStoreTx, Query, ReadError, ReadOptions, SequencedEvent};
use event_store_postgres::PostgresEventStore;
use kv_store::{BatchOp, Key, KvError, KvStoreTx, SetOptions, Value};
use kv_store_postgres::PostgresKvStore;
use secret_store::{CipherAlgorithm, KeyRing, ListSecretOptions, MasterKey, SecretEntry, SecretError, SecretHeader, SecretPath, SecretStoreTx, SecretValue, SetSecretOptions};
use secret_store_postgres::PostgresSecretStore;
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use std::future::Future;
use tracing::{debug, error};

use crate::runner::{IsolationLevel, RetryPolicy, TransactionError, apply_backoff, is_retryable_sql_error};
use crate::uow::UnitOfWork;

/// A unified PostgreSQL Unit of Work bundling multiple stores over a single database transaction.
pub struct PostgresUnitOfWork<'c> {
    tx: Option<Transaction<'c, Postgres>>,
    events: PostgresEventStore,
    kv: PostgresKvStore,
    secrets: PostgresSecretStore,
}

impl<'c> PostgresUnitOfWork<'c> {
    /// Creates a new `PostgresUnitOfWork` wrapping an active SQLx transaction.
    pub fn new(
        tx: Transaction<'c, Postgres>,
        events: PostgresEventStore,
        kv: PostgresKvStore,
        secrets: PostgresSecretStore,
    ) -> Self {
        Self {
            tx: Some(tx),
            events,
            kv,
            secrets,
        }
    }

    /// Access reference to the underlying [`PostgresEventStore`].
    pub fn events(&self) -> &PostgresEventStore {
        &self.events
    }

    /// Access reference to the underlying [`PostgresKvStore`].
    pub fn kv(&self) -> &PostgresKvStore {
        &self.kv
    }

    /// Access reference to the underlying [`PostgresSecretStore`].
    pub fn secrets(&self) -> &PostgresSecretStore {
        &self.secrets
    }

    /// Access mutable reference to the underlying transaction connection handle.
    pub fn conn(&mut self) -> Result<&mut PgConnection, sqlx::Error> {
        self.tx
            .as_mut()
            .map(|t| &mut **t)
            .ok_or_else(|| sqlx::Error::Configuration("Transaction already committed or rolled back".into()))
    }

    // -----------------------------------------------------------------------
    // Event Store convenience operations within this Unit of Work
    // -----------------------------------------------------------------------

    /// Appends events to the Event Store within this active transaction.
    pub async fn append_events(
        &mut self,
        events: &[Event],
        condition: Option<&AppendCondition>,
    ) -> Result<Vec<SequencedEvent>, AppendError> {
        let conn = self
            .tx
            .as_mut()
            .map(|t| &mut **t)
            .ok_or_else(|| AppendError::StoreError("Transaction inactive".into()))?;
        self.events.append_tx(conn, events, condition).await
    }

    /// Reads events from the Event Store within this active transaction.
    pub async fn read_events(
        &mut self,
        query: &Query,
        options: ReadOptions,
    ) -> Result<Vec<SequencedEvent>, ReadError> {
        let conn = self
            .tx
            .as_mut()
            .map(|t| &mut **t)
            .ok_or_else(|| ReadError::StoreError("Transaction inactive".into()))?;
        self.events.read_tx(conn, query, options).await
    }

    // -----------------------------------------------------------------------
    // Key-Value Store convenience operations within this Unit of Work
    // -----------------------------------------------------------------------

    /// Retrieves a value by key within this active transaction.
    pub async fn get_kv(&mut self, key: &Key) -> Result<Option<Value>, KvError> {
        let conn = self
            .tx
            .as_mut()
            .map(|t| &mut **t)
            .ok_or_else(|| KvError::StoreError("Transaction inactive".into()))?;
        self.kv.get_tx(conn, key).await
    }

    /// Sets a key-value entry within this active transaction.
    pub async fn set_kv(
        &mut self,
        key: Key,
        value: Value,
        options: SetOptions,
    ) -> Result<(), KvError> {
        let conn = self
            .tx
            .as_mut()
            .map(|t| &mut **t)
            .ok_or_else(|| KvError::StoreError("Transaction inactive".into()))?;
        self.kv.set_tx(conn, key, value, options).await
    }

    /// Deletes a key within this active transaction.
    pub async fn delete_kv(&mut self, key: &Key) -> Result<bool, KvError> {
        let conn = self
            .tx
            .as_mut()
            .map(|t| &mut **t)
            .ok_or_else(|| KvError::StoreError("Transaction inactive".into()))?;
        self.kv.delete_tx(conn, key).await
    }

    /// Executes a batch of key-value operations within this active transaction.
    pub async fn batch_kv(&mut self, ops: Vec<BatchOp>) -> Result<(), KvError> {
        let conn = self
            .tx
            .as_mut()
            .map(|t| &mut **t)
            .ok_or_else(|| KvError::StoreError("Transaction inactive".into()))?;
        self.kv.batch_tx(conn, ops).await
    }

    // -----------------------------------------------------------------------
    // Secret Store convenience operations within this Unit of Work
    // -----------------------------------------------------------------------

    /// Retrieves a secret by path within this active transaction.
    pub async fn get_secret(&mut self, path: &SecretPath) -> Result<Option<SecretEntry>, SecretError> {
        let conn = self
            .tx
            .as_mut()
            .map(|t| &mut **t)
            .ok_or_else(|| SecretError::StoreError("Transaction inactive".into()))?;
        self.secrets.get_tx(conn, path).await
    }

    /// Stores a new version of a secret within this active transaction.
    pub async fn set_secret(
        &mut self,
        path: SecretPath,
        value: SecretValue,
        options: SetSecretOptions,
    ) -> Result<SecretEntry, SecretError> {
        let conn = self
            .tx
            .as_mut()
            .map(|t| &mut **t)
            .ok_or_else(|| SecretError::StoreError("Transaction inactive".into()))?;
        self.secrets.set_tx(conn, path, value, options).await
    }

    /// Deletes a secret within this active transaction.
    pub async fn delete_secret(&mut self, path: &SecretPath) -> Result<bool, SecretError> {
        let conn = self
            .tx
            .as_mut()
            .map(|t| &mut **t)
            .ok_or_else(|| SecretError::StoreError("Transaction inactive".into()))?;
        self.secrets.delete_tx(conn, path).await
    }

    /// Lists secret headers within this active transaction.
    pub async fn list_secrets(&mut self, options: ListSecretOptions) -> Result<Vec<SecretHeader>, SecretError> {
        let conn = self
            .tx
            .as_mut()
            .map(|t| &mut **t)
            .ok_or_else(|| SecretError::StoreError("Transaction inactive".into()))?;
        self.secrets.list_tx(conn, options).await
    }
}

#[async_trait]
impl<'c> UnitOfWork for PostgresUnitOfWork<'c> {
    type Error = sqlx::Error;

    async fn commit(mut self) -> Result<(), Self::Error> {
        if let Some(tx) = self.tx.take() {
            tx.commit().await?;
        }
        Ok(())
    }

    async fn rollback(mut self) -> Result<(), Self::Error> {
        if let Some(tx) = self.tx.take() {
            tx.rollback().await?;
        }
        Ok(())
    }
}

/// Transaction runner for PostgreSQL, executing units of work with automatic retries and isolation level configuration.
#[derive(Clone)]
pub struct PostgresTransactionRunner {
    pool: PgPool,
    events: PostgresEventStore,
    kv: PostgresKvStore,
    secrets: PostgresSecretStore,
    isolation_level: IsolationLevel,
    retry_policy: RetryPolicy,
}

impl PostgresTransactionRunner {
    /// Creates a new `PostgresTransactionRunner` from explicit store instances.
    pub fn with_stores(
        pool: PgPool,
        events: PostgresEventStore,
        kv: PostgresKvStore,
        secrets: PostgresSecretStore,
    ) -> Self {
        Self {
            pool,
            events,
            kv,
            secrets,
            isolation_level: IsolationLevel::Serializable,
            retry_policy: RetryPolicy::default(),
        }
    }

    /// Creates a new `PostgresTransactionRunner` from a pool and default store configurations.
    pub fn new(pool: PgPool) -> Result<Self, SecretError> {
        let events = PostgresEventStore::new(pool.clone());
        let kv = PostgresKvStore::new(pool.clone());
        let keyring = KeyRing::new(vec![MasterKey::new(1, [7u8; 32])])?;
        let secrets = PostgresSecretStore::new(pool.clone(), keyring, CipherAlgorithm::default());
        Ok(Self::with_stores(pool, events, kv, secrets))
    }

    /// Sets the transaction isolation level for all managed transactions.
    pub fn with_isolation_level(mut self, level: IsolationLevel) -> Self {
        self.isolation_level = level;
        self
    }

    /// Sets the retry policy for transient serialization errors.
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Executes a transactional closure inside a [`PostgresUnitOfWork`].
    /// Automatically commits if the closure returns `Ok`, rolls back on `Err`,
    /// and retries on transient PostgreSQL serialization errors (`40001` or `40P01`).
    pub async fn run<F, Fut, R, E>(&self, mut work: F) -> Result<R, TransactionError<E>>
    where
        F: FnMut(&mut PostgresUnitOfWork<'_>) -> Fut,
        Fut: Future<Output = Result<R, E>>,
    {
        let mut attempt: u32 = 0;

        loop {
            attempt += 1;

            debug!(
                attempt = attempt,
                isolation = ?self.isolation_level,
                "Beginning unit of work transaction"
            );

            let tx = match self.pool.begin_with(self.isolation_level.sql_begin()).await {
                Ok(tx) => tx,
                Err(e) => {
                    let err_msg = e.to_string();
                    if is_retryable_sql_error(&err_msg) && attempt < self.retry_policy.max_attempts {
                        apply_backoff(&self.retry_policy, attempt, &err_msg).await;
                        continue;
                    }
                    return Err(TransactionError::Database(err_msg));
                }
            };

            let mut uow = PostgresUnitOfWork::new(
                tx,
                self.events.clone(),
                self.kv.clone(),
                self.secrets.clone(),
            );

            let result = work(&mut uow).await;

            match result {
                Ok(val) => {
                    if let Err(commit_err) = uow.commit().await {
                        let err_msg = commit_err.to_string();
                        if is_retryable_sql_error(&err_msg) && attempt < self.retry_policy.max_attempts {
                            apply_backoff(&self.retry_policy, attempt, &err_msg).await;
                            continue;
                        }
                        error!(error = %err_msg, "Failed to commit unit of work transaction");
                        return Err(TransactionError::Database(err_msg));
                    }

                    debug!(attempt = attempt, "Unit of work transaction committed successfully");
                    return Ok(val);
                }
                Err(domain_err) => {
                    let _ = uow.rollback().await;
                    return Err(TransactionError::Domain(domain_err));
                }
            }
        }
    }
}
