use event_sourcing::decision::DecisionModels;
use event_sourcing::snapshot::{SnapshotError, SnapshotOptions};
use event_sourcing::{AppendCondition, Event, EventStore, EventStoreTx, ReadError, SequencedEvent};
use kv_store::KvStore;
use thiserror::Error;
use tracing::{debug, error, info};

/// Error type for CQRS Command validation, normalization, model hydration, decision, and persistence steps.
#[derive(Debug, Error)]
pub enum CommandError {
    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Normalization failed: {0}")]
    Normalization(String),

    #[error("Model load failed: {0}")]
    Load(#[from] ReadError),

    #[error("Domain logic decision failed: {0}")]
    Decision(String),

    #[error("Snapshot error: {0}")]
    Snapshot(#[from] SnapshotError),

    #[error("Event store append failed: {0}")]
    Append(#[from] event_sourcing::AppendError),

    #[error("Transaction error: {0}")]
    Transaction(String),
}

/// Generalized trait for CQRS Commands targeting decision models `M`.
///
/// Implementers specify [`validate`](Command::validate) (Step 1), [`normalize`](Command::normalize) (Step 2),
/// [`models`](Command::models) (Step 3 specification), and [`decide`](Command::decide) (Step 4 domain logic).
///
/// Model hydration (Step 3) and atomic event persistence with concurrency boundaries (Step 5)
/// are executed automatically and generically by [`dispatch_command`] or [`dispatch_command_in_tx`].
pub trait Command<M: DecisionModels>: Send + Sync {
    type Error: From<CommandError> + Send + Sync;

    /// Step 1: Validate payload structure, required fields, and format constraints.
    fn validate(&self) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Step 2: Normalize and sanitize payload fields (e.g. trimming strings, lowercasing emails).
    fn normalize(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Step 3 Spec: Specify decision model(s) to be hydrated from the Event Store.
    fn models(&self) -> M;

    /// Step 4: Execute domain logic against the hydrated decision models, returning domain [`Event`]s to commit.
    fn decide(&self, models: &M::Hydrated) -> Result<Vec<Event>, Self::Error>;
}

/// Dispatches a command executing the standard 5-step lifecycle:
/// 1. `command.validate()`
/// 2. `command.normalize()`
/// 3. Hydrates decision models from `event_store`
/// 4. Executes domain decision `command.decide(&loaded.models)`
/// 5. Appends generated events to `event_store` with optimistic concurrency boundary `AppendCondition::new(query).after_opt(...)`
pub async fn dispatch_command<Cmd, M, ES>(
    mut command: Cmd,
    event_store: &ES,
) -> Result<Vec<SequencedEvent>, Cmd::Error>
where
    Cmd: Command<M>,
    M: DecisionModels,
    ES: EventStore,
{
    let command_name = std::any::type_name::<Cmd>();
    debug!(command_name = command_name, "Dispatching command");

    // Step 1: Validate
    if let Err(err) = command.validate() {
        error!(command_name = command_name, "Command validation failed");
        return Err(err);
    }

    // Step 2: Normalize
    if let Err(err) = command.normalize() {
        error!(command_name = command_name, "Command normalization failed");
        return Err(err);
    }

    // Step 3: Hydrate Decision Models from Event Store (AUTOMATIC)
    let loaded = command.models().load_all(event_store).await.map_err(|e| {
        error!(command_name = command_name, error = %e, "Failed to hydrate decision models");
        Cmd::Error::from(CommandError::Load(e))
    })?;

    debug!(
        command_name = command_name,
        max_position = ?loaded.max_position,
        "Decision models hydrated successfully"
    );

    // Step 4: Make Domain Decision
    let events = command.decide(&loaded.models).inspect_err(|_err| {
        error!(command_name = command_name, "Domain decision failed");
    })?;

    if events.is_empty() {
        debug!(
            command_name = command_name,
            "Command generated 0 events; returning empty batch"
        );
        return Ok(Vec::new());
    }

    // Step 5: Save Events into Event Store with Dynamic Concurrency Protection (AUTOMATIC)
    let condition = AppendCondition::new(loaded.combined_query).after_opt(loaded.max_position);

    let appended = event_store
        .append(events, Some(condition))
        .await
        .map_err(|e| {
            error!(command_name = command_name, error = %e, "Failed to append command events to EventStore");
            Cmd::Error::from(CommandError::Append(e))
        })?;

    info!(
        command_name = command_name,
        appended_count = appended.len(),
        "Command executed and events committed successfully"
    );

    Ok(appended)
}

/// Dispatches a command, hydrating decision models with KV Store snapshots if available
/// via `load_decision_model_with_snapshot`, catching up remaining events, and auto-updating snapshots when `snapshot_options.threshold` is reached.
pub async fn dispatch_command_with_snapshot<Cmd, M, ES, KV>(
    mut command: Cmd,
    event_store: &ES,
    kv: &KV,
    snapshot_options: SnapshotOptions,
) -> Result<Vec<SequencedEvent>, Cmd::Error>
where
    Cmd: Command<M>,
    M: DecisionModels,
    ES: EventStore,
    KV: KvStore,
{
    let command_name = std::any::type_name::<Cmd>();
    debug!(
        command_name = command_name,
        threshold = snapshot_options.threshold,
        "Dispatching command with snapshot loading"
    );

    // Step 1: Validate
    if let Err(err) = command.validate() {
        error!(command_name = command_name, "Command validation failed");
        return Err(err);
    }

    // Step 2: Normalize
    if let Err(err) = command.normalize() {
        error!(command_name = command_name, "Command normalization failed");
        return Err(err);
    }

    // Step 3: Hydrate Decision Models using KV Snapshots via load_decision_model_with_snapshot (AUTOMATIC)
    let loaded = command
        .models()
        .load_all_with_snapshot(event_store, kv, snapshot_options)
        .await
        .map_err(|e| {
            error!(command_name = command_name, error = %e, "Failed to hydrate decision models with snapshot");
            Cmd::Error::from(CommandError::Snapshot(e))
        })?;

    debug!(
        command_name = command_name,
        max_position = ?loaded.max_position,
        "Snapshot decision models hydrated successfully"
    );

    // Step 4: Make Domain Decision
    let events = command.decide(&loaded.models).inspect_err(|_err| {
        error!(command_name = command_name, "Domain decision failed");
    })?;

    if events.is_empty() {
        debug!(
            command_name = command_name,
            "Command generated 0 events; returning empty batch"
        );
        return Ok(Vec::new());
    }

    // Step 5: Save Events into Event Store with Dynamic Concurrency Protection (AUTOMATIC)
    let condition = AppendCondition::new(loaded.combined_query).after_opt(loaded.max_position);

    let appended = event_store
        .append(events, Some(condition))
        .await
        .map_err(|e| {
            error!(command_name = command_name, error = %e, "Failed to append command events to EventStore");
            Cmd::Error::from(CommandError::Append(e))
        })?;

    info!(
        command_name = command_name,
        appended_count = appended.len(),
        "Command executed with snapshot and events committed successfully"
    );

    Ok(appended)
}

/// Dispatches a command executing within an active transactional connection `conn: &mut Conn`:
/// 1. `command.validate()`
/// 2. `command.normalize()`
/// 3. Hydrates decision models from `event_store` using `load_all_tx(conn)`
/// 4. Executes domain decision `command.decide(&loaded.models)`
/// 5. Appends generated events to `event_store` via `append_tx(conn, ...)` with optimistic concurrency boundary
pub async fn dispatch_command_tx<Cmd, M, ES, Conn>(
    mut command: Cmd,
    event_store: &ES,
    conn: &mut Conn,
) -> Result<Vec<SequencedEvent>, Cmd::Error>
where
    Cmd: Command<M>,
    M: DecisionModels,
    ES: EventStoreTx<Conn>,
    Conn: Send,
{
    let command_name = std::any::type_name::<Cmd>();
    debug!(
        command_name = command_name,
        "Dispatching command within transaction"
    );

    // Step 1: Validate
    if let Err(err) = command.validate() {
        error!(command_name = command_name, "Command validation failed");
        return Err(err);
    }

    // Step 2: Normalize
    if let Err(err) = command.normalize() {
        error!(command_name = command_name, "Command normalization failed");
        return Err(err);
    }

    // Step 3: Hydrate Decision Models from Event Store within Transaction
    let loaded = command.models().load_all_tx(event_store, conn).await.map_err(|e| {
        error!(command_name = command_name, error = %e, "Failed to hydrate decision models within transaction");
        Cmd::Error::from(CommandError::Load(e))
    })?;

    debug!(
        command_name = command_name,
        max_position = ?loaded.max_position,
        "Decision models hydrated successfully within transaction"
    );

    // Step 4: Make Domain Decision
    let events = command.decide(&loaded.models).inspect_err(|_err| {
        error!(command_name = command_name, "Domain decision failed");
    })?;

    if events.is_empty() {
        debug!(
            command_name = command_name,
            "Command generated 0 events; returning empty batch"
        );
        return Ok(Vec::new());
    }

    // Step 5: Save Events into Event Store with Dynamic Concurrency Protection within Transaction
    let condition = AppendCondition::new(loaded.combined_query).after_opt(loaded.max_position);

    let appended = event_store
        .append_tx(conn, &events, Some(&condition))
        .await
        .map_err(|e| {
            error!(command_name = command_name, error = %e, "Failed to append command events to EventStoreTx");
            Cmd::Error::from(CommandError::Append(e))
        })?;

    info!(
        command_name = command_name,
        appended_count = appended.len(),
        "Command executed and events committed successfully within transaction"
    );

    Ok(appended)
}

/// Dispatches a command executing within an active transactional connection `conn: &mut Conn`,
/// hydrating decision models with KV Store snapshots if available via `load_all_with_snapshot_tx`,
/// catching up remaining events within the transaction, and auto-updating snapshots when `snapshot_options.threshold` is reached.
pub async fn dispatch_command_with_snapshot_tx<Cmd, M, ES, KV, Conn>(
    mut command: Cmd,
    event_store: &ES,
    kv: &KV,
    conn: &mut Conn,
    snapshot_options: SnapshotOptions,
) -> Result<Vec<SequencedEvent>, Cmd::Error>
where
    Cmd: Command<M>,
    M: DecisionModels,
    ES: EventStoreTx<Conn>,
    KV: kv_store::KvStoreTx<Conn>,
    Conn: Send,
{
    let command_name = std::any::type_name::<Cmd>();
    debug!(
        command_name = command_name,
        threshold = snapshot_options.threshold,
        "Dispatching command with snapshot loading within transaction"
    );

    // Step 1: Validate
    if let Err(err) = command.validate() {
        error!(command_name = command_name, "Command validation failed");
        return Err(err);
    }

    // Step 2: Normalize
    if let Err(err) = command.normalize() {
        error!(command_name = command_name, "Command normalization failed");
        return Err(err);
    }

    // Step 3: Hydrate Decision Models using KV Snapshots via load_all_with_snapshot_tx within Transaction (AUTOMATIC)
    let loaded = command
        .models()
        .load_all_with_snapshot_tx(event_store, kv, conn, snapshot_options)
        .await
        .map_err(|e| {
            error!(command_name = command_name, error = %e, "Failed to hydrate decision models with snapshot within transaction");
            Cmd::Error::from(CommandError::Snapshot(e))
        })?;

    debug!(
        command_name = command_name,
        max_position = ?loaded.max_position,
        "Snapshot decision models hydrated successfully within transaction"
    );

    // Step 4: Make Domain Decision
    let events = command.decide(&loaded.models).inspect_err(|_err| {
        error!(command_name = command_name, "Domain decision failed");
    })?;

    if events.is_empty() {
        debug!(
            command_name = command_name,
            "Command generated 0 events; returning empty batch"
        );
        return Ok(Vec::new());
    }

    // Step 5: Save Events into Event Store with Dynamic Concurrency Protection within Transaction (AUTOMATIC)
    let condition = AppendCondition::new(loaded.combined_query).after_opt(loaded.max_position);

    let appended = event_store
        .append_tx(conn, &events, Some(&condition))
        .await
        .map_err(|e| {
            error!(command_name = command_name, error = %e, "Failed to append command events to EventStoreTx");
            Cmd::Error::from(CommandError::Append(e))
        })?;

    info!(
        command_name = command_name,
        appended_count = appended.len(),
        "Command executed with snapshot and events committed successfully within transaction"
    );

    Ok(appended)
}

/// Dispatches a command inside a self-managed database transaction lifecycle:
/// 1. Begins transaction: `tx_provider.begin_tx().await`
/// 2. Validates and normalizes command payload
/// 3. Hydrates decision models from `event_store` within the transaction
/// 4. Executes domain decision `command.decide(&loaded.models)`
/// 5. Appends generated events to `event_store` with optimistic concurrency protection within the transaction
/// 6. Commits transaction: `tx_provider.commit_tx(conn).await`
///
/// If any step or decision fails, the transaction is automatically rolled back via `tx_provider.rollback_tx(conn).await`.
pub async fn dispatch_command_in_tx<Cmd, M, ES, TP>(
    command: Cmd,
    event_store: &ES,
    tx_provider: &TP,
) -> Result<Vec<SequencedEvent>, Cmd::Error>
where
    Cmd: Command<M>,
    M: DecisionModels,
    TP: tx_manager::TransactionProvider,
    ES: EventStoreTx<TP::Conn>,
{
    let mut conn = tx_provider
        .begin_tx()
        .await
        .map_err(|e| Cmd::Error::from(CommandError::Transaction(e.to_string())))?;

    let dispatch_result = dispatch_command_tx(command, event_store, &mut conn).await;

    match dispatch_result {
        Ok(events) => {
            tx_provider
                .commit_tx(conn)
                .await
                .map_err(|e| Cmd::Error::from(CommandError::Transaction(e.to_string())))?;
            Ok(events)
        }
        Err(err) => {
            let _ = tx_provider.rollback_tx(conn).await;
            Err(err)
        }
    }
}

/// Dispatches a command with KV snapshot acceleration inside a self-managed database transaction lifecycle:
/// 1. Begins transaction: `tx_provider.begin_tx().await`
/// 2. Validates and normalizes command payload
/// 3. Hydrates decision models with KV snapshots within the transaction
/// 4. Executes domain decision `command.decide(&loaded.models)`
/// 5. Appends generated events to `event_store` within the transaction
/// 6. Commits transaction: `tx_provider.commit_tx(conn).await`
///
/// If any step or decision fails, the transaction is automatically rolled back via `tx_provider.rollback_tx(conn).await`.
pub async fn dispatch_command_with_snapshot_in_tx<Cmd, M, ES, KV, TP>(
    command: Cmd,
    event_store: &ES,
    kv: &KV,
    tx_provider: &TP,
    snapshot_options: SnapshotOptions,
) -> Result<Vec<SequencedEvent>, Cmd::Error>
where
    Cmd: Command<M>,
    M: DecisionModels,
    TP: tx_manager::TransactionProvider,
    ES: EventStoreTx<TP::Conn>,
    KV: kv_store::KvStoreTx<TP::Conn>,
{
    let mut conn = tx_provider
        .begin_tx()
        .await
        .map_err(|e| Cmd::Error::from(CommandError::Transaction(e.to_string())))?;

    let dispatch_result =
        dispatch_command_with_snapshot_tx(command, event_store, kv, &mut conn, snapshot_options)
            .await;

    match dispatch_result {
        Ok(events) => {
            tx_provider
                .commit_tx(conn)
                .await
                .map_err(|e| Cmd::Error::from(CommandError::Transaction(e.to_string())))?;
            Ok(events)
        }
        Err(err) => {
            let _ = tx_provider.rollback_tx(conn).await;
            Err(err)
        }
    }
}

/// Reusable transactional command dispatcher holding references to the Event Store and Transaction Provider.
pub struct CommandDispatcherTx<ES, TP> {
    event_store: ES,
    tx_provider: TP,
}

impl<ES, TP> CommandDispatcherTx<ES, TP>
where
    TP: tx_manager::TransactionProvider,
    ES: EventStoreTx<TP::Conn>,
{
    /// Creates a new `CommandDispatcherTx`.
    pub fn new(event_store: ES, tx_provider: TP) -> Self {
        Self {
            event_store,
            tx_provider,
        }
    }

    /// Dispatches a command inside a self-managed transaction.
    pub async fn dispatch<Cmd, M>(&self, command: Cmd) -> Result<Vec<SequencedEvent>, Cmd::Error>
    where
        Cmd: Command<M>,
        M: DecisionModels,
    {
        dispatch_command_in_tx(command, &self.event_store, &self.tx_provider).await
    }

    /// Dispatches a command with snapshot acceleration inside a self-managed transaction.
    pub async fn dispatch_with_snapshot<Cmd, M, KV>(
        &self,
        command: Cmd,
        kv: &KV,
        snapshot_options: SnapshotOptions,
    ) -> Result<Vec<SequencedEvent>, Cmd::Error>
    where
        Cmd: Command<M>,
        M: DecisionModels,
        KV: kv_store::KvStoreTx<TP::Conn>,
    {
        dispatch_command_with_snapshot_in_tx(
            command,
            &self.event_store,
            kv,
            &self.tx_provider,
            snapshot_options,
        )
        .await
    }
}
